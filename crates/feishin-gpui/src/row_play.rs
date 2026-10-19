use crate::{app::Feishin, lists::TableRow};
use feishin_core::{
    lists::Column,
    model::{Item, ItemKind},
    queue::Enqueue,
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, Icon, Sizable, StyledExt, button::ButtonVariants};
use std::time::{Duration, Instant};

fn fade_opacity(opened: Instant, now: Instant) -> f32 {
    let mut fade = gpui_component::animation::CssTransition::new(
        0.,
        Duration::from_millis(150),
        [0.25, 0.1, 0.25, 1.],
    );
    fade.retarget(1., opened + Duration::from_millis(150));
    fade.value(now).0
}

pub fn first_number_column(columns: &[Column]) -> Option<&str> {
    columns
        .iter()
        .find(|column| matches!(column.id.as_str(), "rowIndex" | "trackNumber"))
        .map(|column| column.id.as_str())
}
pub fn supports(item: &Item, row: &TableRow) -> bool {
    !matches!(row, TableRow::Queue { .. })
        && matches!(
            item.kind,
            ItemKind::Song
                | ItemKind::Album
                | ItemKind::Artist
                | ItemKind::AlbumArtist
                | ItemKind::Playlist
        )
}
pub fn active(item: &Item, row: &TableRow, current: Option<&feishin_core::queue::Entry>) -> bool {
    let Some(current) = current else {
        return false;
    };
    if let TableRow::Queue { key, .. } = row {
        return *key == current.key;
    }
    match item.kind {
        ItemKind::Album => !item.id.is_empty() && item.id == current.item.album_id,
        ItemKind::Artist | ItemKind::AlbumArtist => {
            !item.id.is_empty()
                && feishin_core::artists::links(&current.item, item.kind == ItemKind::AlbumArtist)
                    .iter()
                    .any(|(id, _)| *id == item.id)
        }
        ItemKind::Playlist => {
            !item.id.is_empty()
                && current.item.context_playlist_id.as_deref() == Some(item.id.as_str())
        }
        _ => current.item.id == item.id,
    }
}

#[derive(Default)]
struct State {
    target: Bounds<Pixels>,
    popup: Bounds<Pixels>,
    viewport: Size<Pixels>,
    opened: Option<Instant>,
    target_hovered: bool,
    popup_hovered: bool,
    pending: Option<Task<()>>,
    focus: Option<FocusHandle>,
    _subscriptions: Vec<Subscription>,
}
impl State {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let handle = window.window_handle();
        let state = cx.weak_entity();
        let escape = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() == handle && event.keystroke.key == "escape" {
                state.update(cx, |state, _| state.close(window)).ok();
            }
        });
        let focus = cx.focus_handle();
        let focused = cx.on_focus_in(&focus, window, |state, window, cx| {
            state.hover(false, true, window, cx);
        });
        Self {
            focus: Some(focus),
            _subscriptions: vec![escape, focused],
            ..Default::default()
        }
    }
    fn close(&mut self, window: &mut Window) {
        self.pending = None;
        self.popup_hovered = false;
        if self.opened.take().is_some() {
            window.refresh();
        }
    }
    fn hover(&mut self, popup: bool, inside: bool, window: &mut Window, cx: &mut Context<Self>) {
        if popup {
            self.popup_hovered = inside;
        } else {
            self.target_hovered = inside;
        }
        self.pending = None;
        if (inside && self.opened.is_some())
            || (!inside && (self.target_hovered || self.popup_hovered))
        {
            return;
        }
        let timer = cx.background_executor().timer(Duration::from_millis(150));
        self.pending = Some(cx.spawn_in(window, async move |this, cx| {
            timer.await;
            this.update_in(cx, |this, window, cx| {
                this.opened = inside.then(|| cx.background_executor().now());
                window.refresh();
            })
            .ok();
        }));
    }
}
#[derive(IntoElement)]
pub struct RowPlay {
    pub id: SharedString,
    pub item: Item,
    pub caption: SharedString,
    pub active: bool,
    pub playing: bool,
    pub enabled: bool,
    pub app: WeakEntity<Feishin>,
}
impl RenderOnce for RowPlay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, State::new);
        state.update(cx, |state, _| {
            if state.viewport != window.viewport_size() {
                state.close(window);
                state.viewport = window.viewport_size();
            }
        });
        let target_state = state.clone();
        let paint_state = state.clone();
        let hover = state.clone();
        let mut target = div()
            .id(self.id.clone())
            .debug_selector({
                let id = self.id.clone();
                move || id.to_string()
            })
            .size_full()
            .relative()
            .h_flex()
            .justify_center()
            .min_h(cx.theme().font_size * 1.5)
            .track_focus(state.read(cx).focus.as_ref().unwrap())
            .tab_index(-1)
            .on_hover(move |inside, window, cx| {
                hover.update(cx, |state, cx| state.hover(false, *inside, window, cx))
            })
            .child(if self.active {
                Icon::default()
                    .path(if self.playing {
                        "native-icons/play.svg"
                    } else {
                        "native-icons/pause.svg"
                    })
                    .with_size(cx.theme().font_size)
                    .text_color(cx.theme().primary)
                    .into_any_element()
            } else {
                div().child(self.caption).into_any_element()
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        target_state.update(cx, |state, _| {
                            if state.target != bounds {
                                state.close(window);
                                state.target = bounds;
                            }
                        });
                    },
                    move |_, _, window, _| {
                        let scroll = paint_state.clone();
                        window.on_mouse_event(move |_: &ScrollWheelEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                scroll.update(cx, |state, _| state.close(window));
                            }
                        });
                        let down = paint_state.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                down.update(cx, |state, _| {
                                    if !state.target.contains(&event.position)
                                        && !state.popup.contains(&event.position)
                                    {
                                        state.close(window);
                                    }
                                });
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            );
        let current = state.read(cx);
        if let Some(opened) = current.opened {
            let zoom = self
                .app
                .read_with(cx, |app, _| app.state.settings.zoom)
                .unwrap_or(1.);
            let position = point(
                current.target.center().x,
                (current.target.top() - px(10. * zoom)).max(px(0.)),
            );
            let elapsed = cx
                .background_executor()
                .now()
                .saturating_duration_since(opened);
            let opacity = fade_opacity(opened, cx.background_executor().now());
            if elapsed < Duration::from_millis(300) {
                window.request_animation_frame();
            }
            let button_size = cx.theme().font_size * 1.875;
            let padding = cx.theme().font_size * 0.5;
            let width = button_size * 3. + padding * 2. + px(4. * zoom);
            let bounds_state = state.clone();
            let enter = state.clone();
            let content = div()
                .id("row-play-popup")
                .relative()
                .role(Role::Toolbar)
                .aria_label(crate::appearance::text("player.play", cx))
                .debug_selector(|| "row-play-popup".into())
                .h_flex()
                .gap_0()
                .p(padding)
                .border(px(2. * zoom))
                .border_color(cx.theme().border)
                .rounded(cx.theme().font_size * 0.3125)
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .shadow(vec![BoxShadow {
                    color: black().opacity(0.4),
                    offset: point(px(2. * zoom), px(2. * zoom)),
                    blur_radius: px(10. * zoom),
                    spread_radius: px(2. * zoom),
                }])
                .foreground_filters(vec![BackdropFilterOperation::DropShadow {
                    offset: [0., 0.],
                    sigma: 5. * zoom,
                    color: Some([0., 0., 0., 0.5]),
                }])
                .opacity(opacity)
                .on_hover(move |inside, window, cx| {
                    enter.update(cx, |state, cx| state.hover(true, *inside, window, cx))
                })
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(RowControls {
                    id: "row-play".into(),
                    item: self.item,
                    enabled: self.enabled,
                    app: self.app,
                })
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, cx| {
                            let center = bounds.center();
                            let radius = px(5. * std::f32::consts::SQRT_2 * zoom);
                            let points = [
                                center - point(px(0.), radius),
                                center + point(radius, px(0.)),
                                center + point(px(0.), radius),
                                center - point(radius, px(0.)),
                            ];
                            let mut fill = PathBuilder::fill();
                            fill.move_to(points[0]);
                            for point in &points[1..] {
                                fill.line_to(*point);
                            }
                            fill.close();
                            if let Ok(path) = fill.build() {
                                window.paint_path(path, cx.theme().background);
                            }
                            let mut border = PathBuilder::stroke(px(2. * zoom));
                            border.move_to(points[1]);
                            border.line_to(points[2]);
                            border.line_to(points[3]);
                            if let Ok(path) = border.build() {
                                window.paint_path(path, cx.theme().border);
                            }
                        },
                    )
                    .absolute()
                    .left(relative(0.5))
                    .ml(px(-5. * zoom))
                    .bottom(px(-7. * zoom))
                    .size(px(10. * zoom)),
                )
                .map(|content| {
                    crate::styles::apply(
                        content,
                        "div",
                        "",
                        &["fs-row-play-control-popover-module-popover"],
                        None,
                        cx,
                    )
                })
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            bounds_state.update(cx, |state, _| state.popup = bounds)
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                );
            target = target.child(
                deferred(
                    anchored()
                        .allow_overflow()
                        .anchor(Corner::BottomLeft)
                        .position(position)
                        .offset(point(-width / 2., px(0.)))
                        .child(content),
                )
                .with_priority(1000),
            );
        }
        target
    }
}

/// Shared Play/Next/Last actions used by table popovers and sidebar playlists.
#[derive(IntoElement)]
pub struct RowControls {
    pub id: SharedString,
    pub item: Item,
    pub enabled: bool,
    pub app: WeakEntity<Feishin>,
}
impl RenderOnce for RowControls {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let target = if self.enabled { 1. } else { 0.5 };
        let opacity = window.use_keyed_state(
            SharedString::from(format!("{}-opacity", self.id)),
            cx,
            |_, _| {
                gpui_component::animation::CssTransition::new(
                    target,
                    Duration::from_millis(200),
                    [0.42, 0., 0.58, 1.],
                )
            },
        );
        let now = cx.background_executor().now();
        let (opacity, animating) = opacity.update(cx, |opacity, _| {
            opacity.retarget(target, now);
            opacity.value(now)
        });
        if animating {
            window.request_animation_frame();
        }
        let hover = cx.theme().background.blend(if cx.theme().mode.is_dark() {
            white().opacity(0.05)
        } else {
            black().opacity(0.05)
        });
        div()
            .id(self.id.clone())
            .h_flex()
            .gap_0()
            .flex_shrink_0()
            .children(
                [
                    (Enqueue::Now, "now", "native-icons/play.svg", "player.play"),
                    (
                        Enqueue::Next,
                        "next",
                        "native-icons/play-next.svg",
                        "player.addNext",
                    ),
                    (
                        Enqueue::End,
                        "last",
                        "native-icons/play-last.svg",
                        "player.addLast",
                    ),
                ]
                .into_iter()
                .map(|(mode, name, icon, label)| {
                    let app = self.app.clone();
                    let item = self.item.clone();
                    let held = item.clone();
                    let held_app = app.clone();
                    let id = format!("{}-{name}", self.id);
                    let mut target = feishin_core::styles::Target::new(
                        "button",
                        &id,
                        &["fs-action-icon-module-root", "mantine-ActionIcon-root"],
                    );
                    target
                        .attributes
                        .insert("data-variant".into(), "subtle".into());
                    if !self.enabled {
                        target
                            .attributes
                            .insert("data-disabled".into(), "true".into());
                    }
                    gpui_component::button::Button::new(SharedString::from(id.clone()))
                        .debug_selector(move || id.clone())
                        .custom(
                            gpui_component::button::ButtonCustomVariant::new(cx)
                                .hover(hover)
                                .active(hover),
                        )
                        .state_style(crate::styles::state_style_path(target, vec![]))
                        .size(cx.theme().font_size * 1.875)
                        .p_0()
                        .disabled(!self.enabled)
                        .text_color(cx.theme().foreground)
                        .opacity(opacity)
                        .icon(
                            Icon::default()
                                .path(icon)
                                .with_size(cx.theme().font_size * 0.875),
                        )
                        .tooltip(format!(
                            "{}\n{}",
                            crate::appearance::text(label, cx),
                            crate::appearance::text("player.holdToShuffle", cx)
                        ))
                        .on_click(move |_, _, cx| {
                            app.update(cx, |app, cx| {
                                app.enqueue_items(vec![item.clone()], mode, false, cx)
                            })
                            .ok();
                            cx.stop_propagation();
                        })
                        .on_long_press(move |_, _, cx| {
                            held_app
                                .update(cx, |app, cx| {
                                    app.enqueue_items(vec![held.clone()], mode, true, cx)
                                })
                                .ok();
                            cx.stop_propagation();
                        })
                }),
            )
    }
}

#[derive(IntoElement)]
pub struct ArtworkPlay {
    pub parent_hovered: Option<bool>,
    pub id: SharedString,
    pub selector: SharedString,
    pub button_selector: SharedString,
    pub item: Item,
    pub row: TableRow,
    pub compact: bool,
    pub width: Option<Pixels>,
    pub height: Pixels,
    pub content: AnyElement,
    pub app: WeakEntity<Feishin>,
}
struct ArtworkState {
    hovered: bool,
    opacity: gpui_component::animation::CssTransition,
}
impl RenderOnce for ArtworkPlay {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| ArtworkState {
            hovered: false,
            opacity: gpui_component::animation::CssTransition::new(
                0.6,
                Duration::from_millis(200),
                [0.42, 0., 0.58, 1.],
            ),
        });
        let hover = state.clone();
        let hovered = self.parent_hovered.unwrap_or(state.read(cx).hovered);
        let (opacity, animating) = state.read(cx).opacity.value(cx.background_executor().now());
        if animating {
            window.request_animation_frame();
        }
        let mut container = div()
            .id(self.id)
            .debug_selector(move || self.selector.to_string())
            .relative()
            .flex_shrink_0()
            .h_flex()
            .justify_center()
            .h(self.height)
            .map(|div| {
                if let Some(width) = self.width {
                    div.w(width)
                } else {
                    div.w_full()
                }
            })
            .on_hover(move |inside, window, cx| {
                hover.update(cx, |state, cx| {
                    state.hovered = *inside;
                    if !inside {
                        state.opacity = gpui_component::animation::CssTransition::new(
                            0.6,
                            Duration::from_millis(200),
                            [0.42, 0., 0.58, 1.],
                        );
                    }
                    window.refresh();
                    cx.notify();
                });
            })
            .child(self.content);
        if hovered {
            let (zoom, behavior, disabled) = self
                .app
                .read_with(cx, |app, _| {
                    (
                        app.state.settings.zoom,
                        app.state.settings.play_button_behavior,
                        !app.library_fetches.is_empty(),
                    )
                })
                .unwrap_or((1., Default::default(), true));
            let play = self.item;
            let held = play.clone();
            let row = self.row;
            let held_row = row.clone();
            let app = self.app;
            let held_app = app.clone();
            let button_size = px(zoom * if self.compact { 24. } else { 32. });
            let button =
                crate::styles::play_control(self.button_selector.clone(), Enqueue::Now, zoom, cx)
                    .debug_selector(move || self.button_selector.to_string())
                    .size(button_size)
                    .disabled(disabled)
                    .tooltip(format!(
                        "{}\n{}",
                        crate::appearance::text(
                            match behavior {
                                feishin_core::queue::PlayBehavior::Next => "player.addNext",
                                feishin_core::queue::PlayBehavior::Last => "player.addLast",
                                feishin_core::queue::PlayBehavior::Shuffle => "player.shuffle",
                                _ => "player.play",
                            },
                            cx
                        ),
                        crate::appearance::text("player.holdToShuffle", cx)
                    ))
                    .when(matches!(row, TableRow::Queue { .. }), |button| {
                        button
                            .without_tooltip()
                            .aria_label(crate::appearance::text("player.play", cx))
                    })
                    .on_click(move |_, _, cx| {
                        app.update(cx, |app, cx| app.play_title_item(&play, &row, false, cx))
                            .ok();
                        cx.stop_propagation();
                    })
                    .on_long_press(move |_, _, cx| {
                        held_app
                            .update(cx, |app, cx| {
                                app.play_title_item(&held, &held_row, true, cx)
                            })
                            .ok();
                        cx.stop_propagation();
                    });
            container = container.child(
                div().absolute().inset_0().h_flex().justify_center().child(
                    div()
                        .id("artwork-play-overlay")
                        .size(button_size)
                        .opacity(opacity)
                        .on_hover(move |inside, window, cx| {
                            state.update(cx, |state, cx| {
                                state.opacity.retarget(
                                    if *inside { 1. } else { 0.6 },
                                    cx.background_executor().now(),
                                );
                                window.refresh();
                            });
                        })
                        .child(button),
                ),
            );
        }
        container
    }
}

#[cfg(test)]
mod tests {
    use super::{active, first_number_column, supports};
    use crate::lists::TableRow;
    use feishin_core::{
        lists::Column,
        model::{Item, ItemKind},
        queue::Entry,
    };

    #[test]
    fn column_priority_and_active_rows_match_the_original_hooks() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/row-controls-reference.json")).unwrap();
        for case in reference["columns"].as_array().unwrap() {
            let columns = case["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| Column {
                    id: id.as_str().unwrap().into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::json!(first_number_column(&columns)),
                case["selected"]
            );
        }
        for case in reference["cases"].as_array().unwrap() {
            let kind = match case["itemType"].as_str().unwrap() {
                "album" => ItemKind::Album,
                "artist" => ItemKind::Artist,
                "albumArtist" => ItemKind::AlbumArtist,
                "playlist" => ItemKind::Playlist,
                "folder" => ItemKind::Folder,
                "genre" => ItemKind::Genre,
                _ => ItemKind::Song,
            };
            let item = Item {
                id: "row".into(),
                kind,
                ..Default::default()
            };
            let row_key = uuid::Uuid::new_v4();
            let row = if case["itemType"] == "queueSong" {
                TableRow::Queue {
                    key: row_key,
                    sidebar: false,
                }
            } else {
                TableRow::Library
            };
            let matched = case["active"].as_bool().unwrap();
            let id = if matched { "row" } else { "different" };
            let current = Entry {
                key: if matched {
                    row_key
                } else {
                    uuid::Uuid::new_v4()
                },
                item: Item {
                    id: id.into(),
                    album_id: id.into(),
                    context_playlist_id: Some(id.into()),
                    artist_links: Some(vec![(id.into(), "Artist".into())]),
                    album_artist_links: Some(vec![(id.into(), "Artist".into())]),
                    ..Default::default()
                },
            };
            assert_eq!(
                supports(&item, &row),
                case["show"].as_bool().unwrap(),
                "{case}"
            );
            let is_active = active(&item, &row, Some(&current));
            assert_eq!(is_active, case["isActive"].as_bool().unwrap(), "{case}");
            assert_eq!(
                is_active && case["playing"].as_bool().unwrap(),
                case["isPlaying"].as_bool().unwrap(),
                "{case}"
            );
        }
    }
    #[gpui::test]
    fn queued_playlist_context_matches_original_data_and_fetch_paths(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::{
            app::{Feishin, Route},
            radio::QueueChange,
        };
        use feishin_core::{
            api::Api,
            model::{Server, ServerType, Session},
            queue::Enqueue,
            storage::State,
        };
        use gpui::AppContext;
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-playlist-context-{}", uuid::Uuid::new_v4()));
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Context fixture".into(),
            url: "http://127.0.0.1:9".into(),
            kind: ServerType::Navidrome,
            username: "fixture".into(),
            legacy_auth: false,
            options: Default::default(),
        };
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.sidebar.playlist_list = false;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            app.update(cx, |app, _| {
                app.sessions.insert(
                    server.id,
                    Api::new(server.clone(), Session::default()).unwrap(),
                );
            });
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/row-controls-reference.json")).unwrap();
        for case in reference["playlistContexts"]
            .as_array()
            .unwrap()
            .iter()
            .chain(reference["dropContexts"].as_array().unwrap())
        {
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.queue_prompt_open = true;
                    app.pending_queue_change = None;
                    app.state.settings.confirm_queue_changes = true;
                    app.route = if case["route"] == "" {
                        Route::Library(ItemKind::Song)
                    } else {
                        Route::Detail(Box::new(Item {
                            id: "current".into(),
                            kind: ItemKind::Playlist,
                            ..Default::default()
                        }))
                    };
                    app.state.queue = Default::default();
                    app.state.queue.add(
                        vec![Item {
                            id: "kept".into(),
                            ..Default::default()
                        }],
                        Enqueue::Now,
                    );
                    let mode = match case["mode"].as_str().unwrap_or("last") {
                        "next" => Enqueue::Next,
                        "last" => Enqueue::End,
                        _ => Enqueue::Now,
                    };
                    let song = Item {
                        id: "song".into(),
                        server_id: server.id,
                        context_playlist_id: case["previous"].as_str().map(str::to_owned),
                        ..Default::default()
                    };
                    if let Some(edge) = case["edge"].as_str() {
                        let items = if case["source"] == "data" {
                            vec![song]
                        } else {
                            (0..case["count"].as_u64().unwrap())
                                .map(|index| Item {
                                    id: format!("source-{index}"),
                                    server_id: server.id,
                                    kind: if case["itemType"] == "playlist" {
                                        ItemKind::Playlist
                                    } else {
                                        ItemKind::Album
                                    },
                                    playlist_songs: vec![song.clone(), song.clone()],
                                    ..Default::default()
                                })
                                .collect()
                        };
                        let dragged = crate::queue::Dragged {
                            sidebar_playlist: false,
                            keys: vec![],
                            items,
                            playlist_source: case["explicit"].as_str().map(|id| {
                                crate::queue::PlaylistSource {
                                    playlist: Item {
                                        id: id.into(),
                                        kind: ItemKind::Playlist,
                                        server_id: server.id,
                                        ..Default::default()
                                    },
                                    entries: vec![],
                                }
                            }),
                        };
                        app.state.queue.position = 17.;
                        app.drop_queue_at(&dragged, app.state.queue.current, edge == "bottom", cx);
                    } else if case["source"] == "data" {
                        if let Some(context) = case["explicit"].as_str() {
                            app.add_tracks_in_context(vec![song], mode, None, Some(context), cx);
                        } else {
                            app.add_tracks(vec![song], mode, cx);
                        }
                    } else {
                        let items = (0..case["count"].as_u64().unwrap())
                            .map(|index| {
                                if case["itemType"] == "playlist" {
                                    Item {
                                        id: format!("source-{index}"),
                                        server_id: server.id,
                                        kind: ItemKind::Playlist,
                                        playlist_songs: vec![song.clone()],
                                        ..Default::default()
                                    }
                                } else {
                                    song.clone()
                                }
                            })
                            .collect::<Vec<_>>();
                        if items.len() == 1 {
                            app.enqueue(items[0].clone(), mode, cx);
                        } else {
                            app.enqueue_items(items, mode, case["mode"] == "shuffle", cx);
                        }
                    }
                })
            });
            cx.run_until_parked();
            app.read_with(cx, |app, _| {
                assert!(app.library_fetches.is_empty(), "Pending fetch: {case}");
                let queued = if let Some(pending) = &app.pending_queue_change {
                    let QueueChange::Replace(songs, _) = &pending.change else {
                        panic!("Wrong confirmation");
                    };
                    songs.clone()
                } else {
                    app.state
                        .queue
                        .entries
                        .iter()
                        .filter(|entry| entry.item.id != "kept")
                        .map(|entry| entry.item.clone())
                        .collect()
                };
                assert!(
                    !queued.is_empty(),
                    "No queued tracks: {case}, {}",
                    app.message
                );
                if let Some(edge) = case["edge"].as_str() {
                    assert_eq!(
                        queued.len(),
                        if case["source"] == "data" {
                            1
                        } else {
                            case["count"].as_u64().unwrap() as usize * 2
                        }
                    );
                    assert_eq!(app.state.queue.current().unwrap().item.id, "kept");
                    assert_eq!(app.state.queue.position, 17.);
                    let ordered = app.state.queue.ordered();
                    assert_eq!(
                        ordered[if edge == "top" { ordered.len() - 1 } else { 0 }]
                            .item
                            .id,
                        "kept"
                    );
                }
                for item in queued {
                    assert_eq!(
                        serde_json::json!(item.context_playlist_id),
                        case["result"],
                        "{case}"
                    );
                    let restored: Item =
                        serde_json::from_value(serde_json::to_value(&item).unwrap()).unwrap();
                    assert_eq!(restored.context_playlist_id, item.context_playlist_id);
                }
            });
        }
        let previous = app.read_with(cx, |app, _| app.state.queue.order.clone());
        app.update(cx, |app, cx| {
            app.drop_queue(
                &crate::queue::Dragged {
                    sidebar_playlist: false,
                    playlist_source: None,
                    keys: vec![],
                    items: vec![Item::default()],
                },
                None,
                cx,
            );
            for cancelled in app.library_fetches.values() {
                cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(app.library_fetches.is_empty());
            assert_eq!(app.state.queue.order, previous);
        });
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[gpui::test]
    fn popover_layout_and_fade_match_original_electron(cx: &mut gpui::TestAppContext) {
        use crate::app::Feishin;
        use feishin_core::storage::State;
        use gpui::{
            AppContext, Context, Entity, Modifiers, Render, Window, div, point, prelude::*, px,
        };
        use gpui_component::Root;
        use std::{
            cell::RefCell,
            rc::Rc,
            time::{Duration, Instant},
        };
        struct Probe {
            app: Entity<Feishin>,
            zoom: f32,
            top: f32,
            index: usize,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().size_full().child(
                    div()
                        .absolute()
                        .left(px(200. * self.zoom))
                        .top(px(self.top * self.zoom))
                        .w(px(80. * self.zoom))
                        .h(px(40. * self.zoom))
                        .child(super::RowPlay {
                            id: format!("layout-target-{}", self.index).into(),
                            item: Item::default(),
                            caption: "1".into(),
                            active: false,
                            playing: false,
                            enabled: true,
                            app: self.app.downgrade(),
                        }),
                )
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-row-layout-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(State::default()), window, cx));
            let probe = cx.new(|_| Probe {
                app: app.clone(),
                zoom: 1.,
                top: 200.,
                index: 0,
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        #[cfg(target_os = "macos")]
        let source = include_str!("../assets/row-controls-layout-darwin-reference.json");
        #[cfg(not(target_os = "macos"))]
        let source = include_str!("../assets/row-controls-layout-linux-reference.json");
        let reference: serde_json::Value = serde_json::from_str(source).unwrap();
        for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let rem = case["rem"].as_f64().unwrap() as f32;
            let top = case["top"].as_f64().unwrap() as f32;
            cx.simulate_mouse_move(point(px(800.), px(600.)), None, Modifiers::none());
            cx.update(|window, cx| {
                app.update(cx, |app, _| app.state.settings.zoom = zoom);
                gpui_component::Theme::global_mut(cx).font_size = px(rem * zoom);
                window.set_rem_size(px(rem * zoom));
                probe.update(cx, |probe, cx| {
                    probe.zoom = zoom;
                    probe.top = top;
                    probe.index = index;
                    cx.notify();
                });
                window.refresh();
            });
            cx.run_until_parked();
            let target = cx.debug_bounds(&format!("layout-target-{index}")).unwrap();
            cx.simulate_mouse_move(target.center(), None, Modifiers::none());
            cx.executor().advance_clock(Duration::from_millis(150));
            cx.run_until_parked();
            let popup = cx.debug_bounds("row-play-popup").unwrap();
            let button = cx.debug_bounds("row-play-now").unwrap();
            for (original, native) in [(0, target), (1, popup), (3, button)] {
                let expected = case["elements"][original]["bounds"].as_array().unwrap();
                for (expected, actual) in expected.iter().zip([
                    native.origin.x,
                    native.origin.y,
                    native.size.width,
                    native.size.height,
                ]) {
                    assert!(
                        (expected.as_f64().unwrap() as f32 - f32::from(actual)).abs() <= 1.,
                        "{case}: {native:?}"
                    );
                }
            }
            let opened = Instant::now();
            for frame in case["frames"].as_array().unwrap() {
                let opacity = super::fade_opacity(
                    opened,
                    opened + Duration::from_millis(frame["milliseconds"].as_u64().unwrap()),
                );
                assert!(
                    (opacity - frame["opacity"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{frame}: {opacity}"
                );
            }
        }
        if dir.exists() {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[gpui::test]
    fn image_column_hover_playback_matches_original_callbacks(cx: &mut gpui::TestAppContext) {
        use crate::{
            app::{Feishin, Route},
            radio::QueueChange,
        };
        use feishin_core::{
            api::Api,
            lists::{Config, Size},
            model::{Server, ServerType, Session},
            queue::{Enqueue, PlayBehavior},
            storage::State,
        };
        use gpui::{
            AppContext, Context, Entity, Modifiers, MouseButton, Render, Role, Window, div, point,
            prelude::*, px,
        };
        use gpui_component::{Root, StyledExt};
        use std::{cell::RefCell, rc::Rc, time::Duration};
        struct Probe {
            app: Entity<Feishin>,
            item: Item,
            row: TableRow,
            config: Config,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .w(px(120.))
                    .h(px(self.config.table.size.row_height()))
                    .h_flex()
                    .child(self.app.update(cx, |app, cx| {
                        app.metadata_cells(
                            &self.item,
                            0,
                            &self.config,
                            vec![Column {
                                id: "imageUrl".into(),
                                width: 120.,
                                ..Default::default()
                            }],
                            self.row.clone(),
                            cx,
                        )
                    }))
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-image-column-{}", uuid::Uuid::new_v4()));
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Image fixture".into(),
            url: "http://127.0.0.1:9".into(),
            kind: ServerType::Navidrome,
            username: "fixture".into(),
            legacy_auth: false,
            options: Default::default(),
        };
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |app, _| {
                app.sessions.insert(
                    server.id,
                    Api::new(server.clone(), Session::default()).unwrap(),
                );
            });
            let probe = cx.new(|_| Probe {
                app: app.clone(),
                item: Item::default(),
                row: TableRow::Library,
                config: Config::default(),
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/row-controls-reference.json")).unwrap();
        let visible = |cx: &mut gpui::VisualTestContext| {
            cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .filter(|(_, node)| {
                        node.role() == Role::Button
                            && !node.is_hidden()
                            && node
                                .label()
                                .is_some_and(|label| label == "Play" || label.contains('\n'))
                    })
                    .count()
            })
        };
        for (index, case) in reference["imageCases"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            cx.simulate_mouse_move(point(px(800.), px(600.)), None, Modifiers::none());
            let queue = case["kind"] == "queueSong";
            let kind = match case["kind"].as_str().unwrap() {
                "album" => ItemKind::Album,
                "artist" => ItemKind::Artist,
                "playlist" => ItemKind::Playlist,
                "folder" => ItemKind::Folder,
                "genre" => ItemKind::Genre,
                _ => ItemKind::Song,
            };
            let item = Item {
                id: format!("case-{index}"),
                name: "Cover".into(),
                kind,
                server_id: if queue { uuid::Uuid::nil() } else { server.id },
                playlist_songs: if kind == ItemKind::Song {
                    vec![]
                } else {
                    (0..3)
                        .map(|n| Item {
                            id: format!("track-{n}"),
                            server_id: server.id,
                            ..Default::default()
                        })
                        .collect()
                },
                ..Default::default()
            };
            let size = [Size::Compact, Size::Default, Size::Large][index % 3];
            cx.update(|window, cx| {
                let row = app.update(cx, |app, cx| {
                    app.queue_prompt_open = true;
                    app.pending_queue_change = None;
                    app.state.settings.confirm_queue_changes = true;
                    app.route = Route::Library(kind);
                    app.state.settings.play_button_behavior =
                        match case["behavior"].as_str().unwrap() {
                            "next" => PlayBehavior::Next,
                            "last" => PlayBehavior::Last,
                            "shuffle" => PlayBehavior::Shuffle,
                            _ => PlayBehavior::Now,
                        };
                    app.state.queue = Default::default();
                    app.state.queue.add(
                        vec![Item {
                            id: "kept".into(),
                            ..Default::default()
                        }],
                        Enqueue::Now,
                    );
                    if queue {
                        app.state.queue.add(vec![item.clone()], Enqueue::End);
                        TableRow::Queue {
                            key: app.state.queue.entries[1].key,
                            sidebar: false,
                        }
                    } else {
                        cx.notify();
                        TableRow::Library
                    }
                });
                probe.update(cx, |probe, cx| {
                    probe.item = item.clone();
                    probe.row = row;
                    probe.config.table.size = size;
                    cx.notify();
                });
                window.refresh();
            });
            cx.run_until_parked();
            assert_eq!(
                visible(cx),
                0,
                "Hidden cover control remains accessible: {case}"
            );
            let artwork = cx
                .debug_bounds(&format!("image-column-artwork-{}", item.id))
                .unwrap();
            assert!(
                (f32::from(artwork.size.height) - (size.row_height() - 8.)).abs() < 1.,
                "{artwork:?}"
            );
            cx.simulate_mouse_move(artwork.center(), None, Modifiers::none());
            cx.run_until_parked();
            assert_eq!(visible(cx), 1, "Missing accessible cover control: {case}");
            let play = cx
                .debug_bounds(&format!("image-column-play-{}", item.id))
                .unwrap();
            let expected = if size == Size::Compact { 24. } else { 32. };
            assert_eq!(f32::from(play.size.width), expected);
            assert_eq!(f32::from(play.size.height), expected);
            if case["held"] == true {
                cx.simulate_mouse_down(play.center(), MouseButton::Left, Modifiers::none());
                cx.run_until_parked();
                cx.executor().advance_clock(Duration::from_millis(501));
                cx.run_until_parked();
                cx.simulate_mouse_up(play.center(), MouseButton::Left, Modifiers::none());
            } else {
                cx.simulate_click(play.center(), Modifiers::none());
            }
            cx.run_until_parked();
            app.read_with(cx, |app, _| {
                assert!(
                    app.library_fetches.is_empty(),
                    "Fetch did not complete: {case}"
                );
                if queue {
                    assert_eq!(app.state.queue.entries.len(), 2);
                    assert_eq!(app.state.queue.current().unwrap().item.id, item.id);
                    assert!(app.pending_queue_change.is_none());
                } else {
                    let replace = !matches!(
                        case["call"]["playType"].as_str(),
                        Some("next" | "nextShuffle" | "last" | "lastShuffle")
                    );
                    assert_eq!(app.pending_queue_change.is_some(), replace, "{case}");
                    let songs = if let Some(pending) = &app.pending_queue_change {
                        let QueueChange::Replace(songs, _) = &pending.change else {
                            panic!("Wrong confirmation")
                        };
                        songs.clone()
                    } else {
                        app.state
                            .queue
                            .entries
                            .iter()
                            .skip(1)
                            .map(|entry| entry.item.clone())
                            .collect()
                    };
                    assert_eq!(
                        songs.len(),
                        if case["call"]["singleSongOnly"] == true {
                            1
                        } else {
                            3
                        },
                        "{case}"
                    );
                }
                assert!(
                    app.selected.is_empty() && app.history.is_empty(),
                    "Cover control changed selection: {case}"
                );
            });
            cx.simulate_mouse_move(point(px(800.), px(600.)), None, Modifiers::none());
            cx.run_until_parked();
            assert_eq!(visible(cx), 0);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn number_cell_hover_opens_delays_closes_and_plays_one_track(cx: &mut gpui::TestAppContext) {
        use crate::app::{Feishin, Route};
        use feishin_core::{queue::Enqueue, storage::State};
        use gpui::{AppContext, Modifiers, MouseButton, Role, VisualTestContext, point, px};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc, time::Duration};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-row-hover-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.confirm_queue_changes = false;
            state.settings.sidebar.playlist_list = false;
            let mut table = feishin_core::lists::default_for("song");
            table.table.columns = ["rowIndex", "trackNumber", "name"]
                .into_iter()
                .map(|id| Column {
                    id: id.into(),
                    width: if id == "name" { 300. } else { 80. },
                    ..Default::default()
                })
                .collect();
            state.settings.lists.insert("song".into(), table);
            state.queue.add(
                vec![Item {
                    id: "kept".into(),
                    ..Default::default()
                }],
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.update(|window, _| window.activate_accessibility_for_test());
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.route = Route::Library(ItemKind::Song);
                app.items = (0..3)
                    .map(|index| Item {
                        id: index.to_string(),
                        name: format!("Song {index}"),
                        track: index + 1,
                        ..Default::default()
                    })
                    .collect();
                cx.notify();
            })
        });
        cx.run_until_parked();
        let visible = |cx: &mut VisualTestContext| {
            cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.role() == Role::Toolbar && node.label() == Some("Play"))
            })
        };
        let target = cx
            .debug_bounds("metadata-cell-0-rowIndex")
            .unwrap()
            .center();
        cx.simulate_mouse_move(target, None, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(149));
        cx.run_until_parked();
        assert!(!visible(cx));
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert!(visible(cx));
        cx.executor().advance_clock(Duration::from_millis(300));
        cx.run_until_parked();
        let next = cx.debug_bounds("row-play-next").unwrap().center();
        cx.simulate_mouse_move(next, None, Modifiers::none());
        cx.simulate_click(next, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(
                app.state
                    .queue
                    .ordered()
                    .iter()
                    .map(|entry| entry.item.id.as_str())
                    .collect::<Vec<_>>(),
                ["kept", "0"]
            );
            assert!(app.selected.is_empty() && app.history.is_empty() && !app.playing);
        });
        let last = cx.debug_bounds("row-play-last").unwrap().center();
        cx.simulate_mouse_move(last, None, Modifiers::none());
        cx.simulate_mouse_down(last, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(visible(cx), "Popup closed when pressing Last");
        cx.executor().advance_clock(Duration::from_millis(501));
        cx.run_until_parked();
        assert!(visible(cx), "Popup closed during the hold");
        assert_eq!(app.read_with(cx, |app, _| app.state.queue.entries.len()), 3);
        cx.simulate_mouse_up(last, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(app.read_with(cx, |app, _| app.state.queue.entries.len()), 3);
        cx.simulate_mouse_move(point(px(900.), px(650.)), None, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(149));
        cx.run_until_parked();
        assert!(visible(cx));
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert!(!visible(cx));
        cx.simulate_mouse_move(target, None, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(150));
        cx.run_until_parked();
        assert!(visible(cx));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!visible(cx));
        for dismiss in ["outside", "scroll", "resize", "remove"] {
            let target = cx
                .debug_bounds("metadata-cell-0-rowIndex")
                .unwrap()
                .center();
            cx.simulate_mouse_move(point(px(900.), px(650.)), None, Modifiers::none());
            cx.simulate_mouse_move(target, None, Modifiers::none());
            cx.executor().advance_clock(Duration::from_millis(150));
            cx.run_until_parked();
            assert!(visible(cx), "Did not reopen before {dismiss}");
            match dismiss {
                "outside" => cx.simulate_click(point(px(900.), px(650.)), Modifiers::none()),
                "scroll" => {
                    cx.simulate_event(gpui::ScrollWheelEvent {
                        position: target,
                        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(2.))),
                        modifiers: Modifiers::none(),
                        touch_phase: gpui::TouchPhase::Moved,
                    });
                }
                "resize" => cx.simulate_resize(gpui::size(px(1100.), px(800.))),
                _ => cx.update(|_, cx| {
                    app.update(cx, |app, cx| {
                        app.items.clear();
                        cx.notify();
                    })
                }),
            }
            cx.run_until_parked();
            assert!(!visible(cx), "Popup survived {dismiss}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
