use crate::scroll::VerticalScroll;
use crate::{
    app::{Feishin, Route},
    queue::Dragged,
};
use feishin_core::panels::Panel;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
    resizable::{ResizableState, h_resizable, resizable_panel, v_resizable},
};

struct SearchMotion {
    width: gpui_component::animation::CssTransition,
    opacity: gpui_component::animation::CssTransition,
}
impl SearchMotion {
    fn new(open: bool) -> Self {
        let value = f32::from(open);
        Self {
            width: gpui_component::animation::CssTransition::new(
                value,
                std::time::Duration::from_millis(300),
                [0.42, 0., 0.58, 1.],
            ),
            opacity: gpui_component::animation::CssTransition::new(
                value,
                std::time::Duration::from_millis(200),
                [0.42, 0., 0.58, 1.],
            ),
        }
    }
    fn values(&mut self, open: bool, now: std::time::Instant) -> (f32, f32, bool) {
        self.width.retarget(f32::from(open), now);
        self.opacity.retarget(f32::from(open), now);
        let (width, resizing) = self.width.value(now);
        let (opacity, fading) = self.opacity.value(now);
        (36. + width * 164., opacity, resizing || fading)
    }
}

#[derive(Clone, Copy)]
enum SearchKind {
    SideQueue,
    Queue,
    Library,
    Settings,
}
impl SearchKind {
    fn input(self, view: &Feishin) -> &Entity<gpui_component::input::InputState> {
        match self {
            Self::SideQueue => &view.side_queue_search,
            Self::Settings => &view.settings_search,
            Self::Queue | Self::Library => &view.local_search,
        }
    }
    fn open(self, view: &Feishin) -> bool {
        match self {
            Self::SideQueue => view.side_queue_search_open,
            Self::Settings => view.settings_search_open,
            Self::Queue | Self::Library => view.queue_search_open,
        }
    }
    fn set_open(self, view: &mut Feishin, open: bool) {
        match self {
            Self::SideQueue => view.side_queue_search_open = open,
            Self::Settings => view.settings_search_open = open,
            Self::Queue | Self::Library => view.queue_search_open = open,
        }
    }
}

#[derive(IntoElement)]
struct SearchControl {
    view: Entity<Feishin>,
    kind: SearchKind,
}
impl RenderOnce for SearchControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.view.update(cx, |view, cx| {
            view.render_list_search_control(self.kind, window, cx)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueueAction {
    Save,
    Restore,
    Shuffle,
    Clear,
    Current,
    Playlist,
}
impl QueueAction {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Save => "player.saveQueueToServer",
            Self::Restore => "player.restoreQueueFromServer",
            Self::Shuffle => "player.shuffle",
            Self::Clear => "action.clearQueue",
            Self::Current => "action.goToCurrent",
            Self::Playlist => "action.createPlaylistFromQueue",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Save => "upload",
            Self::Restore => "download",
            Self::Shuffle => "shuffle",
            Self::Clear => "delete",
            Self::Current => "go-current",
            Self::Playlist => "playlist-add",
        }
    }
    fn id(self, side: bool) -> &'static str {
        if !side {
            return match self {
                Self::Save => "save-server-queue",
                Self::Restore => "load-server-queue",
                Self::Shuffle => "shuffle-queue",
                Self::Clear => "clear-queue",
                Self::Current => "jump-current-queue",
                Self::Playlist => "save-queue-playlist",
            };
        }
        match self {
            Self::Save => "side-queue-save",
            Self::Restore => "side-queue-restore",
            Self::Shuffle => "side-queue-shuffle",
            Self::Clear => "side-queue-clear",
            Self::Current => "side-queue-current",
            Self::Playlist => "side-queue-playlist",
        }
    }
    pub(crate) fn disabled(self, saving: bool, fetching: bool, menu: bool) -> bool {
        match self {
            Self::Save => fetching || menu && saving,
            Self::Restore => saving || fetching,
            _ => false,
        }
    }
    pub(crate) fn loading(self, saving: bool, fetching: bool) -> bool {
        match self {
            Self::Save => saving,
            Self::Restore => fetching,
            _ => false,
        }
    }
}

pub(crate) fn queue_overflow_tier(width: f32) -> u8 {
    if width <= 300. {
        3
    } else if width <= 380. {
        2
    } else if width <= 480. {
        1
    } else {
        0
    }
}
pub(crate) fn queue_toolbar_actions(
    tier: u8,
    supports_queue: bool,
    menu: bool,
) -> Vec<QueueAction> {
    use QueueAction::*;
    [
        (Save, 1),
        (Restore, 1),
        (Shuffle, 3),
        (Clear, 3),
        (Current, 2),
        (Playlist, 2),
    ]
    .into_iter()
    .filter(|(action, threshold)| {
        (supports_queue || !matches!(action, Save | Restore)) && (tier >= *threshold) == menu
    })
    .map(|(action, _)| action)
    .collect()
}

impl Feishin {
    fn run_queue_action(
        &mut self,
        action: QueueAction,
        side: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            QueueAction::Save => self.save_server_queue(cx),
            QueueAction::Restore => self.load_server_queue(cx),
            QueueAction::Shuffle => self.shuffle_queue(cx),
            QueueAction::Clear => self.clear_queue(cx),
            QueueAction::Current => {
                if side {
                    self.scroll_side_queue_to_current(cx);
                } else {
                    self.scroll_queue_to_current();
                }
                cx.notify();
            }
            QueueAction::Playlist => self.create_playlist_from_queue(window, cx),
        }
    }

    fn queue_toolbar_button(
        &self,
        id: &'static str,
        icon: &'static str,
        caption: String,
        cx: &App,
    ) -> Button {
        let rem = f32::from(cx.theme().font_size);
        Button::new(id)
            .debug_selector(move || id.into())
            .ghost()
            .tooltip(caption)
            .icon(
                Icon::default()
                    .path(format!("native-icons/{icon}.svg"))
                    .with_size(px(rem)),
            )
            .loading_icon(
                Icon::default()
                    .path("native-icons/spinner.svg")
                    .with_size(px(rem * 1.125)),
            )
            .loading_icon_size(px(rem * 1.125))
            .size(px(rem * 2.25))
            .min_w(px(rem * 2.25))
            .flex_shrink_0()
            .p_0()
    }

    pub fn list_search_control(
        &self,
        side: bool,
        fill: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        SearchControl {
            view: cx.entity(),
            kind: if side {
                SearchKind::SideQueue
            } else if fill {
                SearchKind::Queue
            } else {
                SearchKind::Library
            },
        }
        .into_any_element()
    }

    pub(crate) fn settings_search_control(&self, cx: &mut Context<Self>) -> AnyElement {
        SearchControl {
            view: cx.entity(),
            kind: SearchKind::Settings,
        }
        .into_any_element()
    }

    fn render_list_search_control(
        &self,
        kind: SearchKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rem = f32::from(cx.theme().font_size);
        let input = kind.input(self);
        let fill = matches!(kind, SearchKind::SideQueue | SearchKind::Queue);
        let border = crate::styles::css_border_width(px(rem / 16.), cx);
        let open = kind.open(self) || !input.read(cx).value().is_empty();
        let (container_id, field_id, clear_id, button_id) = match kind {
            SearchKind::SideQueue => (
                "side-queue-search-container",
                "side-queue-search-field",
                "side-queue-search-clear",
                "side-queue-search",
            ),
            SearchKind::Queue => (
                "queue-search-container",
                "queue-search-field",
                "queue-search-clear",
                "queue-search",
            ),
            SearchKind::Library => (
                "library-search-container",
                "library-search-field",
                "library-search-clear",
                "library-search",
            ),
            SearchKind::Settings => (
                "settings-search-input",
                "settings-search-field",
                "settings-search-clear",
                "settings-search-open",
            ),
        };
        let motion = window.use_keyed_state(container_id, cx, |_, _| SearchMotion::new(open));
        let now = cx.background_executor().now();
        let (width, opacity, animating) = motion.update(cx, |motion, _| motion.values(open, now));
        if animating {
            window.request_animation_frame();
        }
        let field = div()
            .id(field_id)
            .debug_selector(move || field_id.into())
            .relative()
            .opacity(opacity)
            .w_full()
            .min_w_0()
            .when(!fill, |div| div.max_w(window.viewport_size().width * 0.2))
            .child(
                gpui::Styled::h(
                    crate::styles::input(input, cx).with_size(gpui_component::Size::Small),
                    px(rem * 2.25),
                )
                .text_size(px(rem * 13. / 16.))
                .line_height(px(rem * 2.125))
                .pl(px(rem * 0.625))
                .pr(px(if input.read(cx).value().is_empty() {
                    rem * 0.75
                } else {
                    rem * 2.125
                }))
                .gap(px(rem * 0.625))
                .rounded(px(rem * 0.1875))
                .border_l(border)
                .border_r(border)
                .border_t(border)
                .border_b(border)
                .bg(cx.theme().secondary)
                .text_color(cx.theme().secondary_foreground)
                .shadow_none()
                .prefix(
                    Icon::default()
                        .path("native-icons/search.svg")
                        .with_size(px(rem * 0.875)),
                )
                .min_w(
                    px(rem
                        * if input.read(cx).value().is_empty() {
                            2.875
                        } else {
                            4.25
                        })
                        + border * 2.,
                ),
            )
            .when(open && !input.read(cx).value().is_empty(), |div| {
                div.child(
                    self.queue_toolbar_button(clear_id, "close", self.tr("Clear"), cx)
                        .icon(
                            Icon::default()
                                .path("native-icons/close.svg")
                                .with_size(px(rem * 0.8125)),
                        )
                        .absolute()
                        .top_0()
                        .right_0()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let input = kind.input(this).clone();
                            input.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                            cx.notify();
                        })),
                )
            })
            .into_any_element();
        div()
            .id(container_id)
            .debug_selector(move || container_id.into())
            .relative()
            .overflow_hidden()
            .when(fill, |div| div.flex_1())
            .when(!fill, |div| {
                div.flex_shrink_0().w(px(width) * self.state.settings.zoom)
            })
            .min_w_0()
            .h(px(rem * 2.25))
            .on_action(
                cx.listener(move |this, _: &gpui_component::input::Escape, window, cx| {
                    let input = kind.input(this).clone();
                    input.update(cx, |input, cx| input.set_value("", window, cx));
                    kind.set_open(this, false);
                    window.focus(&this.focus);
                    cx.notify();
                }),
            )
            .child(field)
            .child(
                self.queue_toolbar_button(button_id, "search", self.tr("Search"), cx)
                    .absolute()
                    .top_0()
                    .left_0()
                    .when(open, |button| button.mouse_passthrough())
                    .bg(cx.theme().transparent)
                    .text_color(cx.theme().foreground)
                    .opacity(1. - opacity)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        kind.set_open(this, true);
                        let input = kind.input(this).clone();
                        window.defer(cx, move |window, cx| {
                            input.update(cx, |input, cx| input.focus(window, cx));
                        });
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    pub(crate) fn queue_toolbar(&self, side: bool, cx: &mut Context<Self>) -> AnyElement {
        let rem = f32::from(cx.theme().font_size);
        let tier = queue_overflow_tier(if side {
            self.side_queue_toolbar_width
        } else {
            self.queue_toolbar_width
        });
        let supports = self.supports_server_queue();
        let actions = queue_toolbar_actions(tier, supports, false);
        let overflow = queue_toolbar_actions(tier, supports, true);
        let view = cx.entity().downgrade();
        let zoom = self.state.settings.zoom;
        let divider = crate::styles::css_border_width(px(rem / 16.), cx);
        let measure = view.clone();
        div()
            .id(if side {
                "side-queue-toolbar"
            } else {
                "queue-toolbar"
            })
            .debug_selector(move || {
                if side {
                    "side-queue-toolbar"
                } else {
                    "queue-toolbar"
                }
                .into()
            })
            .relative()
            .h_flex()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .items_center()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .px(px(rem * 0.75))
            .py(px(rem * 0.25))
            .gap(px(rem * 0.5))
            .border_b(px(zoom))
            .border_color(cx.theme().border)
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let measure = measure.clone();
                        let width = (f32::from(bounds.size.width) - rem * 1.5) / zoom;
                        window.defer(cx, move |_, cx| {
                            measure
                                .update(cx, |this, cx| {
                                    let measured = if side {
                                        &mut this.side_queue_toolbar_width
                                    } else {
                                        &mut this.queue_toolbar_width
                                    };
                                    if *measured != width {
                                        *measured = width;
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
            .child(
                div()
                    .h_flex()
                    .gap(px(rem * 0.25))
                    .flex_shrink_0()
                    .children(actions.into_iter().map(|action| {
                        self.queue_toolbar_button(
                            action.id(side),
                            action.icon(),
                            self.tr(action.key()),
                            cx,
                        )
                        .disabled(action.disabled(
                            self.queue_save_busy,
                            self.queue_restore_busy,
                            false,
                        ))
                        .loading(action.loading(self.queue_save_busy, self.queue_restore_busy))
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.run_queue_action(action, side, window, cx)
                            },
                        ))
                    }))
                    .when(!overflow.is_empty(), |div| {
                        div.child(
                            self.queue_toolbar_button(
                                if side {
                                    "side-queue-more"
                                } else {
                                    "queue-toolbar-more"
                                },
                                "more",
                                self.tr("action.viewMore"),
                                cx,
                            )
                            .dropdown_menu({
                                let saving = self.queue_save_busy;
                                let fetching = self.queue_restore_busy;
                                let captions = overflow
                                    .iter()
                                    .map(|action| self.tr(action.key()))
                                    .collect::<Vec<_>>();
                                move |mut menu, _, _| {
                                    for (&action, caption) in overflow.iter().zip(&captions) {
                                        let view = view.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(caption.clone())
                                                .icon(Icon::default().path(format!(
                                                    "native-icons/{}.svg",
                                                    action.icon()
                                                )))
                                                .disabled(action.disabled(saving, fetching, true))
                                                .on_click(move |_, window, cx| {
                                                    let view = view.clone();
                                                    window.defer(cx, move |window, cx| {
                                                        view.update(cx, |this, cx| {
                                                            this.run_queue_action(
                                                                action, side, window, cx,
                                                            )
                                                        })
                                                        .ok();
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                }
                            }),
                        )
                    }),
            )
            .child(
                div()
                    .w(divider)
                    .h(px(rem * 1.35))
                    .flex_shrink_0()
                    .bg(cx.theme().border),
            )
            .child(self.list_search_control(side, true, cx))
            .child(
                div()
                    .w(divider)
                    .h(px(rem * 1.35))
                    .flex_shrink_0()
                    .bg(cx.theme().border),
            )
            .child(
                self.queue_toolbar_button(
                    if side {
                        "side-queue-layout"
                    } else {
                        "queue-list-settings"
                    },
                    "settings",
                    self.tr("List layout"),
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit_list_config(
                        if side {
                            "sideQueue".into()
                        } else {
                            this.list_key()
                        },
                        window,
                        cx,
                    )
                })),
            )
            .into_any_element()
    }
    pub fn active_panels(&self) -> Vec<Panel> {
        if self.is_full_player_view() || self.visualizer_display.mounted {
            return vec![];
        }
        self.state.settings.panels.enabled(
            self.route != Route::Queue,
            true,
            self.route != Route::Visualizer,
        )
    }
    pub fn panels_visualizer_visible(&self) -> bool {
        let config = &self.state.settings.panels;
        config.kind == "sideQueue"
            && config.expanded
            && self.active_panels().iter().any(|panel| {
                *panel == Panel::Visualizer
                    || (*panel == Panel::Lyrics
                        && config.combined
                        && config.visualizer
                        && self.route != Route::Visualizer)
            })
    }
    pub fn toggle_playback_panels(&mut self, cx: &mut Context<Self>) {
        if self.state.settings.panels.kind == "sideDrawerQueue" {
            self.queue_drawer = !self.queue_drawer;
        } else {
            self.state.settings.panels.expanded = !self.state.settings.panels.expanded;
            self.queue_drawer = false;
            self.save();
        }
        cx.notify();
    }
    pub fn side_queue_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut config = self
            .state
            .settings
            .lists
            .get("sideQueue")
            .cloned()
            .unwrap_or_else(|| feishin_core::lists::default_for("sideQueue"));
        if config.display == "detail" {
            config.table = config.detail.clone().unwrap_or(config.table);
        }
        let mut columns = config
            .table
            .columns
            .iter()
            .filter(|column| {
                column.is_enabled
                    && !(column.id == "albumGroup" && crate::queue::album_grouping(&config))
            })
            .cloned()
            .collect::<Vec<_>>();
        columns.sort_by_key(|column| match column.pinned.as_deref() {
            Some("left") => 0,
            Some("right") => 2,
            _ => 1,
        });
        let entries = self
            .side_queue_rows()
            .into_iter()
            .map(|(index, entry)| (index, entry.clone()))
            .collect::<Vec<_>>();
        let empty = entries.is_empty();
        let row_config = config.clone();
        let row_columns = columns.clone();
        let rows = if crate::queue::album_grouping(&config) || self.queue_has_sections(true) {
            let display = self.queue_display_rows(
                &entries.iter().map(|(_, entry)| entry).collect::<Vec<_>>(),
                &config,
                true,
            );
            if self.side_queue_group_scroll.item_count() != display.len() {
                self.side_queue_group_scroll.reset(display.len());
            }
            let entity = cx.entity();
            list(self.side_queue_group_scroll.clone(), move |row, _, cx| {
                entity.update(cx, |this, cx| match &display[row] {
                    crate::queue::GroupedQueueRow::Section { up_next, count } => {
                        this.queue_section_header(*up_next, *count, row, true, cx)
                    }
                    crate::queue::GroupedQueueRow::Album(range) => {
                        let songs = entries[range.clone()]
                            .iter()
                            .map(|(_, entry)| entry.item.clone())
                            .collect::<Vec<_>>();
                        this.queue_album_header(
                            &songs,
                            entries[range.start].1.key,
                            entries[range.start].0,
                            true,
                            cx,
                        )
                    }
                    crate::queue::GroupedQueueRow::Track(index) => {
                        let (index, entry) = &entries[*index];
                        this.side_queue_row(entry, *index, &row_config, &row_columns, cx)
                    }
                })
            })
            .flex_1()
            .min_h_0()
            .into_any_element()
        } else {
            uniform_list("side-queue-list", entries.len(), {
                let entity = cx.entity();
                move |range, _, cx| {
                    entity.update(cx, |this, cx| {
                        range
                            .map(|i| {
                                let (index, entry) = &entries[i];
                                this.side_queue_row(entry, *index, &row_config, &row_columns, cx)
                            })
                            .collect()
                    })
                }
            })
            .track_scroll(self.side_queue_scroll.clone())
            .flex_1()
            .min_h_0()
            .into_any_element()
        };
        let rows = div()
            .relative()
            .v_flex()
            .flex_1()
            .min_h_0()
            .child(rows)
            .child(
                if crate::queue::album_grouping(&config) || self.queue_has_sections(true) {
                    crate::scroll::Scrollbar::vertical(
                        "side-queue-scrollbar",
                        &self.side_queue_group_scroll,
                    )
                } else {
                    crate::scroll::Scrollbar::vertical(
                        "side-queue-scrollbar",
                        &self.side_queue_scroll,
                    )
                },
            )
            .child(
                if crate::queue::album_grouping(&config) || self.queue_has_sections(true) {
                    crate::scroll::Scrollbar::horizontal(
                        "side-queue-horizontal-scrollbar",
                        &self.side_queue_horizontal_scroll,
                    )
                    .in_viewport(&self.side_queue_group_scroll)
                } else {
                    crate::scroll::Scrollbar::horizontal(
                        "side-queue-horizontal-scrollbar",
                        &self.side_queue_horizontal_scroll,
                    )
                    .in_viewport(&self.side_queue_scroll)
                },
            );
        div()
            .id("side-queue")
            .debug_selector(|| "side-queue".into())
            .v_flex()
            .gap_2()
            .size_full()
            .on_drop(cx.listener(|this, dragged: &Dragged, _, cx| {
                this.drop_queue(dragged, None, cx);
                cx.stop_propagation();
            }))
            .child(self.queue_toolbar(true, cx))
            .when(config.table.enable_header, |div| {
                let cells = columns
                    .iter()
                    .cloned()
                    .map(|column| {
                        let label = crate::lists::table_header(&column, cx).into_any_element();
                        let label =
                            self.column_drag_controls_for("sideQueue", &column, label, false, cx);
                        (column, label)
                    })
                    .collect();
                div.child(
                    gpui::div()
                        .id("side-queue-header")
                        .debug_selector(|| "side-queue-header".into())
                        .h_flex()
                        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                        .w_full()
                        .h(px(40. * self.state.settings.zoom))
                        .px_2()
                        .gap_2()
                        .child(
                            gpui::div()
                                .w(px(18. * self.state.settings.zoom))
                                .flex_shrink_0(),
                        )
                        .child(self.column_strip(
                            "side-queue-header-scroll".into(),
                            cells,
                            &config.table,
                            false,
                            &self.side_queue_horizontal_scroll,
                            cx,
                        ))
                        .child(
                            gpui::div()
                                .w(px(32. * self.state.settings.zoom))
                                .flex_shrink_0(),
                        ),
                )
            })
            .child(
                div()
                    .id("side-queue-rows")
                    .key_context("SideQueueList")
                    .track_focus(&self.side_queue_focus)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|view, _, window, _| window.focus(&view.side_queue_focus)),
                    )
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .when(empty, |div| div.child(self.tr("Queue is empty")))
                    .child(rows),
            )
            .into_any_element()
    }
    fn side_queue_row(
        &mut self,
        entry: &feishin_core::queue::Entry,
        index: usize,
        config: &feishin_core::lists::Config,
        columns: &[feishin_core::lists::Column],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.fetch_item_cover(&entry.item, cx);
        let key = entry.key;

        div()
            .id(SharedString::from(format!("side-queue-row-{index}")))
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                this.hover_table_row(
                    crate::lists::TableRow::Queue { key, sidebar: true },
                    index,
                    *hovered,
                    cx,
                )
            }))
            .debug_selector(move || format!("side-queue-row-{index}"))
            .h_flex()
            .w_full()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .items_center()
            .gap_2()
            .h(px(config.table.size.row_height() * self.state.settings.zoom))
            .px_2()
            .rounded_md()
            .when(config.table.enable_horizontal_borders, |div| {
                div.border_b_1().border_color(cx.theme().border)
            })
            .bg(crate::styles::row_background(
                config.table.enable_alternate_row_colors && index % 2 == 1,
                self.side_queue_selected.contains(&key),
                false,
                cx,
            ))
            .when(config.table.enable_row_hover_highlight, |div| {
                div.hover(|style| {
                    style.bg(crate::styles::row_background(
                        config.table.enable_alternate_row_colors && index % 2 == 1,
                        self.side_queue_selected.contains(&key),
                        true,
                        cx,
                    ))
                })
            })
            .relative()
            .on_drag(
                self.drag_queue_entry(key, true),
                crate::context_preview::drag_constructor(cx),
            )
            .on_drop_with_bounds(crate::queue::row_drop_listener(
                cx,
                move |this, dragged, after, _, cx| {
                    this.drop_queue_at(dragged, Some(key), after, cx);
                    cx.stop_propagation();
                },
            ))
            .child(
                Checkbox::new(SharedString::from(format!("side-queue-select-{key}")))
                    .w(px(18. * self.state.settings.zoom))
                    .flex_shrink_0()
                    .checked(self.side_queue_selected.contains(&key))
                    .on_click(cx.listener(move |this, _: &bool, _, cx| {
                        this.select_queue_row(
                            key,
                            true,
                            Modifiers {
                                control: true,
                                ..Modifiers::none()
                            },
                            cx,
                        );
                    })),
            )
            .child(self.metadata_cells(
                &entry.item,
                index,
                config,
                columns.to_vec(),
                crate::lists::TableRow::Queue { key, sidebar: true },
                cx,
            ))
            .child(
                crate::styles::button(SharedString::from(format!("side-queue-remove-{key}")), cx)
                    .w(px(32. * self.state.settings.zoom))
                    .px_0()
                    .flex_shrink_0()
                    .ghost()
                    .label(crate::appearance::text("×", cx))
                    .tooltip(crate::appearance::text("Remove track", cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_queue_entries(&[key], cx);
                    })),
            )
            .context_menu(self.queue_menu_builder(entry, true, cx))
            .into_any_element()
    }

    pub fn main_queue_visible(&self) -> bool {
        if self.full_player {
            self.state.settings.player_display.active_tab == "queue"
        } else {
            self.route == Route::Queue
        }
    }

    pub fn scroll_queue_to_current(&mut self) {
        let entries = self.queue_rows();
        let Some(index) = entries
            .iter()
            .position(|entry| Some(entry.key) == self.state.queue.current)
        else {
            return;
        };
        let config = self.list_config();
        let (entries, index) = if config.pagination == "paginated" {
            self.library_page = index / config.items_per_page.max(1);
            let start = self.library_page * config.items_per_page.max(1);
            (
                &entries[start..entries.len().min(start + config.items_per_page.max(1))],
                index - start,
            )
        } else {
            (entries.as_slice(), index)
        };
        if crate::queue::album_grouping(&config) || self.queue_has_sections(false) {
            let rows = self.queue_display_rows(&entries.iter().collect::<Vec<_>>(), &config, false);
            if self.queue_group_scroll.item_count() != rows.len() {
                self.queue_group_scroll.reset(rows.len());
            }
            if let Some(row) = rows
                .iter()
                .position(|row| *row == crate::queue::GroupedQueueRow::Track(index))
            {
                self.queue_group_scroll.scroll_to(ListOffset {
                    item_ix: row,
                    offset_in_item: px(0.),
                });
            }
        } else {
            self.queue_scroll
                .scroll_to_item(index, ScrollStrategy::Center);
        }
    }

    pub fn side_queue_visible(&self) -> bool {
        !self.full_player
            && (self.queue_drawer
                || (self.state.settings.panels.expanded
                    && self.active_panels().contains(&Panel::Queue)))
    }

    pub fn sync_queue_follow(&mut self, cx: &mut Context<Self>) {
        if !self.state.settings.follow_current_song {
            self.queue_follow_signature = (None, false, false);
            return;
        }
        let main = self.main_queue_visible();
        let side = self.side_queue_visible();
        let signature = (self.state.queue.current, main, side);
        if self.queue_follow_signature == signature {
            return;
        }
        self.queue_follow_signature = signature;
        if self.state.queue.current.is_none() {
            return;
        }
        if main {
            self.scroll_queue_to_current();
        }
        if side {
            self.scroll_side_queue_to_current(cx);
        }
    }

    pub fn scroll_side_queue_to_current(&mut self, _cx: &mut Context<Self>) {
        let Some(current) = self.state.queue.current else {
            return;
        };
        let entries = self
            .side_queue_rows()
            .into_iter()
            .map(|(_, entry)| entry)
            .collect::<Vec<_>>();
        if let Some(index) = entries.iter().position(|entry| entry.key == current) {
            let config = self
                .state
                .settings
                .lists
                .get("sideQueue")
                .cloned()
                .unwrap_or_else(|| feishin_core::lists::default_for("sideQueue"));
            if crate::queue::album_grouping(&config) || self.queue_has_sections(true) {
                let rows = self.queue_display_rows(&entries, &config, true);
                if self.side_queue_group_scroll.item_count() != rows.len() {
                    self.side_queue_group_scroll.reset(rows.len());
                }
                if let Some(row) = rows
                    .iter()
                    .position(|row| *row == crate::queue::GroupedQueueRow::Track(index))
                {
                    self.side_queue_group_scroll.scroll_to(ListOffset {
                        item_ix: row,
                        offset_in_item: px(0.),
                    });
                }
            } else {
                self.side_queue_scroll
                    .scroll_to_item(index, ScrollStrategy::Center);
            }
        }
    }

    fn playback_panel_view(
        &mut self,
        panel: Panel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = match panel {
            Panel::Queue => self.side_queue_view(cx),
            Panel::Lyrics => {
                if self.state.settings.panels.combined
                    && self.state.settings.panels.visualizer
                    && self.route != Route::Visualizer
                {
                    let visualizer = self.visualizer_surface(true, false, window, cx);
                    let rem = f32::from(cx.theme().font_size);
                    div()
                        .relative()
                        .size_full()
                        .child(
                            div()
                                .id("combined-visualizer-background")
                                .debug_selector(|| "combined-visualizer-background".into())
                                .absolute()
                                .inset_0()
                                .opacity(
                                    if self.state.settings.panels.lyrics && !self.lyrics.is_empty()
                                    {
                                        0.2
                                    } else {
                                        1.
                                    },
                                )
                                .child(visualizer),
                        )
                        .when(self.state.settings.panels.lyrics, |div| {
                            div.child(
                                gpui::div()
                                    .relative()
                                    .size_full()
                                    .p(px(rem * 0.75))
                                    .child(self.lyrics_surface_for("sidebar", window, cx)),
                            )
                        })
                        .into_any_element()
                } else {
                    self.lyrics_surface_for("sidebar", window, cx)
                }
            }
            Panel::Visualizer => self.visualizer_surface(true, true, window, cx),
        };
        let rem = f32::from(cx.theme().font_size);
        let hover = window.use_keyed_state(
            SharedString::from(format!("playback-panel-hover-{panel:?}")),
            cx,
            |_, _| {
                gpui_component::animation::CssTransition::new(
                    0.,
                    std::time::Duration::from_millis(200),
                    [0.25, 0.1, 0.25, 1.],
                )
            },
        );
        let (opacity, animating) = hover.read(cx).value(cx.background_executor().now());
        if animating {
            window.request_animation_frame();
        }
        let index = self
            .state
            .settings
            .panels
            .order
            .iter()
            .position(|value| *value == panel)
            .unwrap_or(0);
        div()
            .id(SharedString::from(format!("playback-panel-{panel:?}")))
            .debug_selector(move || format!("playback-panel-{panel:?}"))
            .relative()
            .v_flex()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .bg(if panel == Panel::Queue {
                cx.theme().background
            } else {
                cx.theme().muted
            })
            .when(
                panel == Panel::Lyrics
                    && !(self.state.settings.panels.combined
                        && self.state.settings.panels.visualizer
                        && self.route != Route::Visualizer),
                |div| div.p(px(rem * 0.75)),
            )
            .on_hover(move |hovered, _, cx| {
                hover.update(cx, |hover, cx| {
                    let target = f32::from(*hovered);
                    if hover.target() != target {
                        hover.retarget(target, cx.background_executor().now());
                        cx.notify();
                    }
                })
            })
            .when(
                (panel == Panel::Visualizer
                    || panel == Panel::Lyrics && self.state.settings.panels.combined)
                    && !self.state.settings.mpv_path.is_empty(),
                |div| div.child(self.system_audio_control(cx)),
            )
            .child(div().flex_1().min_h_0().w_full().child(content))
            .when(panel != Panel::Queue, |div| {
                div.child(
                    gpui::div()
                        .absolute()
                        .top(px(rem * 0.75))
                        .left(px(rem * 0.75))
                        .h_flex()
                        .opacity(opacity)
                        .children(
                            [
                                (true, gpui_component::IconName::ArrowUp, "action.moveUp"),
                                (
                                    false,
                                    gpui_component::IconName::ArrowDown,
                                    "action.moveDown",
                                ),
                            ]
                            .into_iter()
                            .map(|(up, icon, key)| {
                                let id = SharedString::from(format!(
                                    "panel-hover-{panel:?}-{}",
                                    if up { "up" } else { "down" }
                                ));
                                let selector = id.to_string();
                                crate::styles::button(id, cx)
                                    .debug_selector(move || selector.clone())
                                    .ghost()
                                    .icon(Icon::new(icon).with_size(px(rem * 0.8125)))
                                    .tooltip(self.tr(key))
                                    .size(px(rem * 1.875))
                                    .p_0()
                                    .disabled(if up {
                                        index == 0
                                    } else {
                                        index + 1 >= self.state.settings.panels.order.len()
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(index) = this
                                            .state
                                            .settings
                                            .panels
                                            .order
                                            .iter()
                                            .position(|value| *value == panel)
                                        {
                                            this.state.settings.panels.move_panel(index, up);
                                            this.save();
                                            cx.notify();
                                        }
                                    }))
                            }),
                        )
                        .child(
                            crate::styles::button(
                                SharedString::from(format!("panel-hover-{panel:?}-close")),
                                cx,
                            )
                            .debug_selector(move || format!("panel-hover-{panel:?}-close"))
                            .ghost()
                            .icon(
                                Icon::default()
                                    .path("native-icons/close.svg")
                                    .with_size(px(rem * 0.8125)),
                            )
                            .tooltip(self.tr("common.close"))
                            .size(px(rem * 1.875))
                            .p_0()
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.state.settings.panels.close_panel(panel);
                                    this.save();
                                    cx.notify();
                                },
                            )),
                        ),
                )
            })
            .into_any_element()
    }

    pub fn wrap_playback_panels(
        &mut self,
        content: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let config = self.state.settings.panels.clone();
        let panels = self.active_panels();
        if config.kind != "sideQueue" || !config.expanded || panels.is_empty() {
            return content;
        }
        let vertical = config.layout == "vertical";
        let panel_sizes = panels
            .iter()
            .enumerate()
            .map(|(index, panel)| config.panel_size(*panel, index, &panels))
            .collect::<Vec<_>>();
        let children = panels
            .into_iter()
            .enumerate()
            .map(|(index, panel)| {
                resizable_panel()
                    .when_some(panel_sizes[index], |panel, size| panel.size(px(size)))
                    .size_range(px(0.)..px(1600.))
                    .child(self.playback_panel_view(panel, window, cx))
            })
            .collect::<Vec<_>>();
        let inside = v_resizable("playback-panels-split")
            .auto_fit(px(1.))
            .children(children)
            .on_resize(cx.listener(|this, state: &Entity<ResizableState>, _, cx| {
                this.state.settings.panels.sizes = state
                    .read(cx)
                    .sizes()
                    .iter()
                    .map(|size| f32::from(*size).clamp(40., 1600.))
                    .collect();
                this.save();
            }));
        let pane = div()
            .id("playback-panels")
            .debug_selector(|| "playback-panels".into())
            .v_flex()
            .size_full()
            .min_h_0()
            .bg(cx.theme().secondary)
            .child(inside);
        let group = if vertical {
            v_resizable("playback-bottom-split")
        } else {
            h_resizable("playback-right-split")
        };
        let available = if vertical {
            f32::from(window.viewport_size().height) - 210.
        } else {
            f32::from(window.viewport_size().width) - self.state.settings.sidebar.width - 280.
        };
        let size = if vertical {
            config.height
        } else {
            config.width
        }
        .min(available.max(160.));
        group
            .child(resizable_panel().child(content))
            .child(
                resizable_panel()
                    .size(px(size))
                    .size_range(px(160.)..px(1600.))
                    .child(pane),
            )
            .on_resize(
                cx.listener(move |this, state: &Entity<ResizableState>, _, cx| {
                    if let Some(size) = state.read(cx).sizes().last() {
                        if vertical {
                            this.state.settings.panels.height = f32::from(*size).clamp(160., 1200.);
                        } else {
                            this.state.settings.panels.width = f32::from(*size).clamp(240., 1600.);
                        }
                        this.save();
                    }
                }),
            )
            .into_any_element()
    }
    pub fn queue_drawer_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.full_player || self.visualizer_display.mounted || self.route == Route::Queue {
            return Empty.into_any_element();
        }
        if self.queue_drawer {
            return div()
                .absolute()
                .top_4()
                .bottom(px(120.))
                .right_4()
                .w(px(450.))
                .max_w_full()
                .p_3()
                .v_flex()
                .gap_2()
                .shadow_lg()
                .rounded_lg()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .id("queue-drawer")
                .debug_selector(|| "queue-drawer".into())
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    this.queue_drawer_generation += 1;
                    let generation = this.queue_drawer_generation;
                    if !*hovered {
                        cx.spawn(async move |this, cx| {
                            smol::Timer::after(std::time::Duration::from_millis(50)).await;
                            this.update(cx, |this, cx| {
                                if this.queue_drawer_generation == generation
                                    && !cx.has_active_drag()
                                {
                                    this.queue_drawer = false;
                                    cx.notify();
                                }
                            })
                            .ok();
                        })
                        .detach();
                    }
                }))
                .child(
                    crate::styles::button("drawer-close", cx)
                        .label(crate::appearance::text("Close queue", cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.queue_drawer = false;
                            this.queue_drawer_generation += 1;
                            cx.notify();
                        })),
                )
                .child(self.side_queue_view(cx))
                .into_any_element();
        }
        if self.state.settings.panels.expanded {
            return Empty.into_any_element();
        }
        div()
            .absolute()
            .right_0()
            .top(px(180.))
            .id("queue-drawer-edge")
            .debug_selector(|| "queue-drawer-edge".into())
            .child(
                crate::styles::button("drawer-open", cx)
                    .ghost()
                    .label(crate::appearance::text("‹", cx))
                    .tooltip(crate::appearance::text("Queue", cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.queue_drawer = true;
                        cx.notify();
                    })),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.queue_drawer_generation += 1;
                let generation = this.queue_drawer_generation;
                if *hovered {
                    cx.spawn(async move |this, cx| {
                        smol::Timer::after(std::time::Duration::from_millis(500)).await;
                        this.update(cx, |this, cx| {
                            if this.queue_drawer_generation == generation {
                                this.queue_drawer = true;
                                cx.notify();
                            }
                        })
                        .ok();
                    })
                    .detach();
                }
            }))
            .into_any_element()
    }
    pub fn panel_settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.state.settings.panels;
        div()
            .id("panel-settings-view")
            .v_flex()
            .size_full()
            .native_vertical_scroll()
            .gap_3()
            .child(
                crate::styles::button("panels-type", cx)
                    .label(format!(
                        "Queue: {}",
                        if config.kind == "sideQueue" {
                            "Resizable panels"
                        } else {
                            "Drawer"
                        }
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.panels.kind =
                            if this.state.settings.panels.kind == "sideQueue" {
                                "sideDrawerQueue"
                            } else {
                                "sideQueue"
                            }
                            .into();
                        this.queue_drawer = false;
                        this.save();
                        cx.notify();
                    })),
            )
            .child(
                crate::styles::button("panels-layout", cx)
                    .label(format!(
                        "Panels: {}",
                        if config.layout == "horizontal" {
                            "Right"
                        } else {
                            "Bottom"
                        }
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.panels.layout =
                            if this.state.settings.panels.layout == "horizontal" {
                                "vertical"
                            } else {
                                "horizontal"
                            }
                            .into();
                        this.save();
                        cx.notify();
                    })),
            )
            .children(
                [
                    (0, "Show queue", config.queue),
                    (1, "Show lyrics", config.lyrics),
                    (2, "Show visualizer", config.visualizer),
                    (3, "Combine lyrics and visualizer", config.combined),
                ]
                .into_iter()
                .map(|(index, label, checked)| {
                    div()
                        .debug_selector(move || format!("panel-option-{index}"))
                        .child(
                            Checkbox::new(("panel-option", index as usize))
                                .label(label)
                                .checked(checked)
                                .on_click(cx.listener(move |this, value: &bool, _, cx| {
                                    let config = &mut this.state.settings.panels;
                                    match index {
                                        0 => config.queue = *value,
                                        1 => config.lyrics = *value,
                                        2 => config.visualizer = *value,
                                        _ => config.combined = *value,
                                    }
                                    this.save();
                                    cx.notify();
                                })),
                        )
                }),
            )
            .children(config.order.iter().enumerate().map(|(index, panel)| {
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .items_center()
                    .child(div().w(px(160.)).child(self.tr(panel.label())))
                    .child(
                        div()
                            .debug_selector(move || format!("panel-up-{index}"))
                            .child(
                                crate::styles::button(("panel-up", index), cx)
                                    .ghost()
                                    .label(crate::appearance::text("↑", cx))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.state.settings.panels.move_panel(index, true);
                                        this.save();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        crate::styles::button(("panel-down", index), cx)
                            .ghost()
                            .label(crate::appearance::text("↓", cx))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.state.settings.panels.move_panel(index, false);
                                this.save();
                                cx.notify();
                            })),
                    )
            }))
            .child(
                crate::styles::button("panel-reset", cx)
                    .label(crate::appearance::text("Reset panels", cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.panels = Default::default();
                        this.save();
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{queue_overflow_tier, queue_toolbar_actions};
    use crate::app::{Feishin, Route};
    use feishin_core::{
        api::Api,
        model::{Item, Server, ServerType, Session},
        queue::Enqueue,
        storage::State,
    };
    use gpui::{
        AppContext, Context, Entity, Focusable, IntoElement, Modifiers, Render, Role, Subscription,
        TestAppContext, Window, div, prelude::*, px, size,
    };
    use gpui_component::ActiveTheme;
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn sidebar_panel_sizes_match_original_rendered_geometry(cx: &mut TestAppContext) {
        use feishin_core::panels::{Config, Panel};
        use gpui_component::resizable::{resizable_panel, v_resizable};
        struct Probe {
            config: Config,
            height: f32,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().w(px(600.)).h(px(self.height)).child(
                    v_resizable("panel-size-probe").auto_fit(px(1.)).children(
                        self.config.order.iter().enumerate().map(|(index, panel)| {
                            resizable_panel()
                                .size_range(px(0.)..px(1600.))
                                .when_some(
                                    self.config.panel_size(*panel, index, &self.config.order),
                                    |panel, value| panel.size(px(value)),
                                )
                                .child(
                                    div()
                                        .id(gpui::SharedString::from(format!(
                                            "panel-size-{panel:?}"
                                        )))
                                        .debug_selector(move || format!("panel-size-{panel:?}"))
                                        .size_full()
                                        .min_h_0(),
                                )
                        }),
                    ),
                )
            }
        }
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let probe = cx.new(|_| Probe {
                config: Config::default(),
                height: 320.,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        #[cfg(target_os = "macos")]
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/sidebar-panel-layout-darwin-reference.json"
        ))
        .unwrap();
        #[cfg(not(target_os = "macos"))]
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/sidebar-panel-layout-linux-reference.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let order: Vec<Panel> = serde_json::from_value(case["order"].clone()).unwrap();
            cx.update(|_, cx| {
                probe.update(cx, |probe, cx| {
                    probe.config.order = order;
                    probe.config.sizes = if case["saved"].is_null() {
                        vec![]
                    } else {
                        serde_json::from_value(case["saved"].clone()).unwrap()
                    };
                    probe.height = case["height"].as_f64().unwrap() as f32;
                    cx.notify();
                });
            });
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            for expected in case["panels"].as_array().unwrap() {
                let panel: Panel = serde_json::from_value(expected["panel"].clone()).unwrap();
                let bounds = cx.debug_bounds(&format!("panel-size-{panel:?}")).unwrap();
                assert!(
                    (f32::from(bounds.origin.y) - expected["y"].as_f64().unwrap() as f32).abs()
                        <= 1.,
                    "{case}: {bounds:?}"
                );
                assert!(
                    (f32::from(bounds.size.height) - expected["height"].as_f64().unwrap() as f32)
                        .abs()
                        <= 1.,
                    "{case}: {bounds:?}"
                );
            }
        }
    }

    #[test]
    fn queue_toolbar_matches_original_overflow_and_busy_states() {
        let references: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/queue-toolbar-reference.json")).unwrap();
        assert_eq!(references.len(), 112);
        for case in references {
            let tier = queue_overflow_tier(case["width"].as_f64().unwrap() as f32);
            assert_eq!(u64::from(tier), case["tier"].as_u64().unwrap());
            let supports = case["supportsQueue"].as_bool().unwrap();
            let saving = case["saving"].as_bool().unwrap();
            let fetching = case["fetching"].as_bool().unwrap();
            let buttons = queue_toolbar_actions(tier, supports, false);
            let menu = queue_toolbar_actions(tier, supports, true);
            let mut actual=buttons.iter().map(|action|serde_json::json!({"key":action.key(),"disabled":action.disabled(saving,fetching,false),"loading":action.loading(saving,fetching)})).collect::<Vec<_>>();
            if !menu.is_empty() {
                actual.push(serde_json::json!({"key":"more","disabled":false,"loading":false}));
            }
            assert_eq!(serde_json::json!(actual), case["buttons"], "{case}");
            let actual=menu.iter().map(|action|serde_json::json!({"key":action.key(),"disabled":action.disabled(saving,fetching,true)})).collect::<Vec<_>>();
            assert_eq!(serde_json::json!(actual), case["menu"], "{case}");
        }
    }

    #[gpui::test]
    fn queue_toolbars_resize_keep_actions_and_preserve_duplicate_playlist_order(
        cx: &mut TestAppContext,
    ) {
        use std::{
            io::{BufRead, BufReader, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                let body = if request.contains("ping") {
                    r#"{"subsonic-response":{"status":"ok","serverVersion":"0.57.0"}}"#
                } else {
                    assert!(request.contains("getOpenSubsonicExtensions"), "{request}");
                    r#"{"subsonic-response":{"status":"ok","openSubsonicExtensions":[]}}"#
                };
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            }
        });
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Queue fixture".into(),
            url: format!("http://{address}"),
            username: "fixture".into(),
            kind: ServerType::Navidrome,
            legacy_auth: false,
            options: Default::default(),
        };
        let api = Api::new(server.clone(), Session::default()).unwrap();
        api.discover_features();
        worker.join().unwrap();
        assert!(api.features().native_queue);
        struct Probe {
            app: Entity<Feishin>,
            _subscription: Subscription,
            width: f32,
            side: bool,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div().w(px(self.width)).child(
                    self.app
                        .update(cx, |app, cx| app.queue_toolbar(self.side, cx)),
                )
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-queue-toolbar-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.confirm_queue_changes = false;
            state.queue.add(
                (0..80)
                    .map(|index| Item {
                        id: format!("duplicate-{}", index % 5),
                        name: format!("Track {index}"),
                        server_id: server.id,
                        ..Default::default()
                    })
                    .collect(),
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|cx| Probe {
                app: app.clone(),
                _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
                width: 624.,
                side: true,
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1800.), px(700.)));
        cx.run_until_parked();
        #[cfg(target_os = "macos")]
        let layouts: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/queue-toolbar-layout-darwin-reference.json"
        ))
        .unwrap();
        #[cfg(not(target_os = "macos"))]
        let layouts: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/queue-toolbar-layout-linux-reference.json"
        ))
        .unwrap();
        for (root_size, zoom) in [(16., 1.), (16., 1.5), (24., 1.), (24., 1.5)] {
            for side in [false, true] {
                for supports in [false, true] {
                    for width in [240., 300., 301., 380., 381., 480., 481., 600.] {
                        cx.update(|window, cx| {
                            app.update(cx, |app, cx| {
                                app.state.settings.zoom = zoom;
                                app.state.active_server = supports.then_some(server.id);
                                app.sessions.clear();
                                if supports {
                                    app.sessions.insert(server.id, api.clone());
                                }
                                app.apply_theme(window, cx);
                                gpui_component::Theme::global_mut(cx).font_size =
                                    px(root_size * zoom);
                                cx.notify();
                            });
                            let rem = f32::from(cx.theme().font_size);
                            probe.update(cx, |probe, cx| {
                                probe.width = width * zoom + rem * 1.5;
                                probe.side = side;
                                cx.notify();
                            });
                        });
                        cx.run_until_parked();
                        let measured = app.read_with(cx, |app, _| {
                            if side {
                                app.side_queue_toolbar_width
                            } else {
                                app.queue_toolbar_width
                            }
                        });
                        assert!(
                            (measured - width).abs() < 0.75,
                            "{width} at {zoom}: {measured}"
                        );
                        let tier = queue_overflow_tier(width);
                        let actions = queue_toolbar_actions(tier, supports, false);
                        let overflow = queue_toolbar_actions(tier, supports, true);
                        let tree = cx.update(|window, _| {
                            window.accessibility_tree_for_test().unwrap().clone()
                        });
                        let labels = tree
                            .nodes
                            .iter()
                            .filter(|(_, node)| node.role() == Role::Button)
                            .filter_map(|(_, node)| node.label())
                            .collect::<Vec<_>>();
                        let mut expected = actions
                            .iter()
                            .map(|action| app.read_with(cx, |app, _| app.tr(action.key())))
                            .collect::<Vec<_>>();
                        let reference = layouts["cases"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|case| {
                                case["rootSize"].as_f64() == Some(f64::from(root_size))
                                    && case["width"].as_f64() == Some(f64::from(width))
                                    && case["supportsQueue"] == supports
                                    && case["mode"] == "closed"
                            })
                            .unwrap();
                        let toolbar = cx
                            .debug_bounds(if side {
                                "side-queue-toolbar"
                            } else {
                                "queue-toolbar"
                            })
                            .unwrap();
                        let compare =
                            |actual: gpui::Bounds<gpui::Pixels>, expected: &serde_json::Value| {
                                let values = [
                                    f32::from(actual.origin.x - toolbar.origin.x),
                                    f32::from(actual.origin.y - toolbar.origin.y),
                                    f32::from(actual.size.width),
                                    f32::from(actual.size.height),
                                ];
                                for (&actual, expected) in
                                    values.iter().zip(expected.as_array().unwrap())
                                {
                                    assert!(
                                        (actual - expected.as_f64().unwrap() as f32 * zoom).abs()
                                            <= 1.1,
                                        "{actual} vs {expected} at {zoom}; {reference}"
                                    );
                                }
                            };
                        compare(toolbar, &reference["toolbar"]);
                        compare(
                            cx.debug_bounds(if side {
                                "side-queue-search-container"
                            } else {
                                "queue-search-container"
                            })
                            .unwrap(),
                            &reference["search"],
                        );
                        for button in reference["buttons"].as_array().unwrap() {
                            let id = match button["icon"].as_str().unwrap() {
                                "upload" => super::QueueAction::Save.id(side),
                                "download" => super::QueueAction::Restore.id(side),
                                "mediaShuffle" => super::QueueAction::Shuffle.id(side),
                                "delete" => super::QueueAction::Clear.id(side),
                                "goToItem" => super::QueueAction::Current.id(side),
                                "playlistAdd" => super::QueueAction::Playlist.id(side),
                                "ellipsisHorizontal" => {
                                    if side {
                                        "side-queue-more"
                                    } else {
                                        "queue-toolbar-more"
                                    }
                                }
                                "search" => {
                                    if side {
                                        "side-queue-search"
                                    } else {
                                        "queue-search"
                                    }
                                }
                                "settings" => {
                                    if side {
                                        "side-queue-layout"
                                    } else {
                                        "queue-list-settings"
                                    }
                                }
                                other => panic!("Unexpected original icon: {other}"),
                            };
                            compare(cx.debug_bounds(id).unwrap(), &button["bounds"]);
                        }
                        if !overflow.is_empty() {
                            expected.push(app.read_with(cx, |app, _| app.tr("action.viewMore")));
                        }
                        expected.push(app.read_with(cx, |app, _| app.tr("Search")));
                        expected.push(app.read_with(cx, |app, _| app.tr("List layout")));
                        assert_eq!(
                            labels,
                            expected.iter().map(String::as_str).collect::<Vec<_>>(),
                            "{width}, {zoom}, side={side}, supports={supports}"
                        );
                        for action in actions {
                            let bounds = cx.debug_bounds(action.id(side)).unwrap();
                            assert!(
                                (f32::from(bounds.size.width) - root_size * 2.25 * zoom).abs()
                                    < 0.75,
                                "{bounds:?}"
                            );
                        }
                        if !overflow.is_empty() {
                            let bounds = cx
                                .debug_bounds(if side {
                                    "side-queue-more"
                                } else {
                                    "queue-toolbar-more"
                                })
                                .unwrap();
                            cx.simulate_click(bounds.center(), Modifiers::none());
                            cx.run_until_parked();
                            let tree = cx.update(|window, _| {
                                window.accessibility_tree_for_test().unwrap().clone()
                            });
                            let labels = tree
                                .nodes
                                .iter()
                                .filter(|(_, node)| node.role() == Role::MenuItem)
                                .map(|(_, node)| {
                                    node.children()
                                        .iter()
                                        .find_map(|id| {
                                            tree.nodes
                                                .iter()
                                                .find(|(child, _)| child == id)
                                                .and_then(|(_, child)| child.value())
                                        })
                                        .unwrap_or_default()
                                })
                                .collect::<Vec<_>>();
                            let expected = overflow
                                .iter()
                                .map(|action| app.read_with(cx, |app, _| app.tr(action.key())))
                                .collect::<Vec<_>>();
                            assert_eq!(
                                labels,
                                expected.iter().map(String::as_str).collect::<Vec<_>>()
                            );
                            cx.simulate_keystrokes("escape");
                            cx.run_until_parked();
                        }
                    }
                }
            }
        }
        for side in [false, true] {
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.route = Route::Queue;
                    cx.notify();
                });
                probe.update(cx, |probe, cx| {
                    probe.side = side;
                    cx.notify();
                })
            });
            cx.run_until_parked();
            let bounds = cx
                .debug_bounds(if side {
                    "side-queue-search"
                } else {
                    "queue-search"
                })
                .unwrap();
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
            cx.update(|window, cx| {
                app.read_with(cx, |app, cx| {
                    let input = if side {
                        &app.side_queue_search
                    } else {
                        &app.local_search
                    };
                    assert!(
                        input.read(cx).focus_handle(cx).is_focused(window),
                        "queue search did not retain focus, side={side}, open={}",
                        if side {
                            app.side_queue_search_open
                        } else {
                            app.queue_search_open
                        }
                    );
                })
            });
            cx.simulate_input("Track 7");
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.run_until_parked();
            app.read_with(cx, |app, cx| {
                let input = if side {
                    &app.side_queue_search
                } else {
                    &app.local_search
                };
                assert_eq!(input.read(cx).value().as_str(), "Track 7");
                if !side {
                    assert_eq!(app.local_filter, "Track 7");
                }
            });
            let clear = cx
                .debug_bounds(if side {
                    "side-queue-search-clear"
                } else {
                    "queue-search-clear"
                })
                .unwrap();
            cx.simulate_click(clear.center(), Modifiers::none());
            cx.run_until_parked();
            cx.update(|window, cx| {
                app.read_with(cx, |app, cx| {
                    let input = if side {
                        &app.side_queue_search
                    } else {
                        &app.local_search
                    };
                    assert!(input.read(cx).value().is_empty());
                    assert!(input.read(cx).focus_handle(cx).is_focused(window));
                })
            });
            cx.simulate_input("Track 7");
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.run_until_parked();
            let more = cx
                .debug_bounds(if side {
                    "side-queue-more"
                } else {
                    "queue-toolbar-more"
                })
                .unwrap();
            cx.simulate_click(more.center(), Modifiers::none());
            cx.run_until_parked();
            app.read_with(cx, |app, cx| {
                let input = if side {
                    &app.side_queue_search
                } else {
                    &app.local_search
                };
                assert_eq!(input.read(cx).value().as_str(), "Track 7");
            });
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
            let bounds = cx
                .debug_bounds(if side {
                    "side-queue-search-container"
                } else {
                    "queue-search-container"
                })
                .unwrap();
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.run_until_parked();
            app.read_with(cx, |app, cx| {
                let input = if side {
                    &app.side_queue_search
                } else {
                    &app.local_search
                };
                assert!(input.read(cx).value().is_empty());
                assert!(
                    !(if side {
                        app.side_queue_search_open
                    } else {
                        app.queue_search_open
                    })
                );
                assert!(app.local_filter.is_empty());
            });
        }
        // Menu actions must use the queue's current order, including duplicate IDs.
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.state.active_server = None;
                app.state.settings.zoom = 1.;
                app.apply_theme(window, cx);
                cx.notify();
            });
            probe.update(cx, |probe, cx| {
                probe.width = 300.;
                probe.side = true;
                cx.notify();
            });
        });
        cx.run_until_parked();
        let keys = app.read_with(cx, |app, _| app.state.queue.order.clone());
        let current = app.read_with(cx, |app, _| app.state.queue.current);
        let bounds = cx.debug_bounds("side-queue-more").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(app.state.queue.current, current);
            assert_ne!(app.state.queue.order, keys);
            let mut actual = app.state.queue.order.clone();
            let mut expected = keys.clone();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected);
        });
        let songs = app.read_with(cx, |app, _| {
            app.state
                .queue
                .ordered()
                .iter()
                .map(|entry| entry.item.clone())
                .collect::<Vec<_>>()
        });
        let bounds = cx.debug_bounds("side-queue-more").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
        cx.simulate_keystrokes("down down down down enter");
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(app.route, Route::PlaylistEditor);
            assert_eq!(app.form_songs, songs);
        });
        let restored = feishin_core::storage::load(&dir.join("state.json")).unwrap();
        assert_eq!(
            restored.queue.order,
            app.read_with(cx, |app, _| app.state.queue.order.clone())
        );
        std::fs::remove_dir_all(dir).ok();
    }
    #[gpui::test]
    fn shared_search_layout_matches_original_viewports_fonts_and_clear_controls(
        cx: &mut TestAppContext,
    ) {
        use feishin_core::model::ItemKind;
        use gpui::{Bounds, Pixels};
        use gpui_component::StyledExt;
        struct Probe {
            app: Entity<Feishin>,
            fill: bool,
            _subscription: Subscription,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div().h_flex().items_start().w_full().child(
                    self.app
                        .update(cx, |app, cx| app.list_search_control(false, self.fill, cx)),
                )
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-search-layout-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|cx| Probe {
                app: app.clone(),
                fill: false,
                _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        #[cfg(target_os = "macos")]
        let reference = include_str!("../assets/list-search-layout-darwin-reference.json");
        #[cfg(not(target_os = "macos"))]
        let reference = include_str!("../assets/list-search-layout-linux-reference.json");
        let reference: serde_json::Value = serde_json::from_str(reference).unwrap();
        for zoom in [1., 1.5] {
            for case in reference["cases"].as_array().unwrap() {
                let fill = case["fill"].as_bool().unwrap();
                let mode = case["mode"].as_str().unwrap();
                let root = case["rootSize"].as_f64().unwrap() as f32;
                cx.simulate_resize(size(
                    px(case["viewport"].as_f64().unwrap() as f32 * zoom),
                    px(600.),
                ));
                cx.update(|window, cx| {
                    app.update(cx, |app, cx| {
                        app.route = Route::Library(ItemKind::Song);
                        app.queue_search_open = false;
                        app.state.settings.zoom = zoom;
                        app.apply_theme(window, cx);
                        gpui_component::Theme::global_mut(cx).font_size = px(root * zoom);
                        app.local_filter = if mode == "shortcut" { "Existing" } else { "" }.into();
                        app.local_search.update(cx, |input, cx| {
                            input.set_value(
                                if mode == "shortcut" { "Existing" } else { "" },
                                window,
                                cx,
                            )
                        });
                        window.focus(&app.focus);
                        cx.notify();
                    });
                    probe.update(cx, |probe, cx| {
                        probe.fill = fill;
                        cx.notify();
                    });
                });
                cx.run_until_parked();
                cx.executor()
                    .advance_clock(std::time::Duration::from_millis(350));
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
                let id = if fill {
                    "queue-search"
                } else {
                    "library-search"
                };
                if mode == "shortcut" {
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.run_shortcut(
                                feishin_core::shortcuts::Action::LocalSearch,
                                window,
                                cx,
                            )
                        })
                    });
                } else if mode != "closed" {
                    let point = cx.debug_bounds(id).unwrap().center();
                    cx.simulate_click(point, Modifiers::none());
                }
                cx.run_until_parked();
                if mode == "open" && !fill && root == 16. && case["viewport"] == 1200 {
                    #[cfg(target_os = "macos")]
                    let motion = include_str!("../assets/list-search-motion-darwin-reference.json");
                    #[cfg(not(target_os = "macos"))]
                    let motion = include_str!("../assets/list-search-motion-linux-reference.json");
                    let motion: serde_json::Value = serde_json::from_str(motion).unwrap();
                    let mut previous_ms = 0;
                    for frame in motion["motion"][0]["samples"].as_array().unwrap() {
                        let ms = frame["milliseconds"].as_u64().unwrap();
                        cx.executor()
                            .advance_clock(std::time::Duration::from_millis(ms - previous_ms));
                        cx.update(|window, _| window.refresh());
                        cx.run_until_parked();
                        previous_ms = ms;
                        let bounds = cx.debug_bounds("library-search-container").unwrap();
                        assert!(
                            (f64::from(bounds.size.width) / f64::from(zoom)
                                - frame["width"].as_f64().unwrap())
                            .abs()
                                < 1.1,
                            "Rendered search width: {frame}; {bounds:?}"
                        );
                        let glyphs =
                            cx.update(|window, _| window.painted_glyphs_with_opacity_for_test());
                        for (icon_size, opacity) in [(14., "inputOpacity"), (16., "buttonOpacity")]
                        {
                            let actual = glyphs
                                .iter()
                                .filter(|(bounds, _, _)| {
                                    (f32::from(bounds.size.height) - icon_size * zoom).abs() < 0.1
                                        && (f32::from(bounds.size.width) - icon_size * zoom).abs()
                                            < 0.1
                                })
                                .map(|(_, _, color)| color.a)
                                .fold(0_f32, f32::max);
                            assert!(
                                (f64::from(actual) - frame[opacity].as_f64().unwrap()).abs()
                                    < 0.001,
                                "Rendered search {opacity}: {frame}; {glyphs:?}"
                            );
                        }
                    }
                }
                cx.executor()
                    .advance_clock(std::time::Duration::from_millis(350));
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
                if ["filled-blur", "clear", "escape"].contains(&mode) {
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.local_filter = "Track 7".into();
                            view.local_search
                                .update(cx, |input, cx| input.set_value("Track 7", window, cx));
                            if mode == "filled-blur" {
                                window.focus(&view.focus);
                            }
                            cx.notify();
                        })
                    });
                    cx.run_until_parked();
                    cx.executor()
                        .advance_clock(std::time::Duration::from_millis(350));
                    cx.update(|window, _| window.refresh());
                    cx.run_until_parked();
                }
                if mode == "clear" {
                    let point = cx.debug_bounds(&format!("{id}-clear")).unwrap().center();
                    cx.simulate_click(point, Modifiers::none());
                } else if mode == "escape" {
                    cx.simulate_keystrokes("escape");
                }
                cx.run_until_parked();
                cx.executor()
                    .advance_clock(std::time::Duration::from_millis(350));
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
                let compare = |actual: Bounds<Pixels>, expected: &serde_json::Value| {
                    for (actual, expected) in [
                        f32::from(actual.origin.x),
                        f32::from(actual.origin.y),
                        f32::from(actual.size.width),
                        f32::from(actual.size.height),
                    ]
                    .into_iter()
                    .zip(expected.as_array().unwrap())
                    {
                        assert!(
                            (actual - expected.as_f64().unwrap() as f32 * zoom).abs() <= 1.1,
                            "{actual} vs {expected} at {zoom}: {case}"
                        );
                    }
                };
                compare(
                    cx.debug_bounds(&format!("{id}-container")).unwrap(),
                    &case["container"],
                );
                if case["inputOpacity"] == "1" {
                    let quads = cx.update(|window, _| {
                        window
                            .painted_lines_and_quads_for_test()
                            .into_iter()
                            .map(|(bounds, color, border, other)| {
                                (
                                    bounds.map(|v| px(f64::from(v) as f32 / window.scale_factor())),
                                    color,
                                    border,
                                    other,
                                )
                            })
                            .collect::<Vec<_>>()
                    });
                    let bounds = quads
                        .iter()
                        .filter(|(bounds, _, _, _)| {
                            f32::from(bounds.origin.x).abs() < 1.1
                                && (f32::from(bounds.size.height) - root * zoom * 2.25).abs() < 1.1
                        })
                        .max_by(|a, b| {
                            f32::from(a.0.size.width).total_cmp(&f32::from(b.0.size.width))
                        })
                        .unwrap_or_else(|| panic!("Missing input paint: {case}; quads={quads:?}"))
                        .0;
                    compare(bounds, &case["input"]);
                }
                for button in case["buttons"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|button| button["opacity"] == "1")
                {
                    let selector = if button["icon"] == "x" {
                        format!("{id}-clear")
                    } else {
                        id.to_owned()
                    };
                    compare(cx.debug_bounds(&selector).unwrap(), &button["bounds"]);
                }
                cx.update(|window, cx| {
                    app.read_with(cx, |view, cx| {
                        assert_eq!(
                            view.local_search.read(cx).value().as_str(),
                            case["value"].as_str().unwrap()
                        );
                        assert_eq!(
                            view.local_search
                                .read(cx)
                                .focus_handle(cx)
                                .is_focused(window),
                            case["focused"].as_bool().unwrap()
                        );
                    })
                });
                if mode == "filled-blur" {
                    // The transparent search button overlaps the input prefix. Its
                    // pointer behavior must leave the underlying field reachable.
                    cx.simulate_click(
                        gpui::point(px(3. * zoom), px(root * zoom)),
                        Modifiers::none(),
                    );
                    cx.run_until_parked();
                    cx.update(|window, cx| {
                        app.read_with(cx, |view, cx| {
                            assert!(
                                view.local_search
                                    .read(cx)
                                    .focus_handle(cx)
                                    .is_focused(window)
                            );
                            assert_eq!(view.local_search.read(cx).value().as_str(), "Track 7");
                        });
                    });
                }
                if mode == "open" && root == 16. && case["viewport"] == 1200 {
                    // CSS pointer-events:none keeps the original button in the tab
                    // order, including activation that restores input focus.
                    cx.simulate_keystrokes("tab");
                    cx.run_until_parked();
                    cx.update(|window, cx| {
                        app.read_with(cx, |view, cx| {
                            assert!(
                                !view
                                    .local_search
                                    .read(cx)
                                    .focus_handle(cx)
                                    .is_focused(window)
                            );
                        });
                    });
                    cx.simulate_keystrokes("enter");
                    cx.simulate_event(gpui::KeyUpEvent {
                        keystroke: gpui::Keystroke::parse("enter").unwrap(),
                    });
                    cx.run_until_parked();
                    cx.update(|window, cx| {
                        app.read_with(cx, |view, cx| {
                            assert!(
                                view.local_search
                                    .read(cx)
                                    .focus_handle(cx)
                                    .is_focused(window)
                            );
                        });
                    });
                }
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn shared_search_motion_matches_original_interrupted_transitions() {
        #[cfg(target_os = "macos")]
        let reference = include_str!("../assets/list-search-motion-darwin-reference.json");
        #[cfg(not(target_os = "macos"))]
        let reference = include_str!("../assets/list-search-motion-linux-reference.json");
        let reference: serde_json::Value = serde_json::from_str(reference).unwrap();
        for case in reference["motion"].as_array().unwrap() {
            let mut opening = case["direction"] == "open";
            let mut state = super::SearchMotion::new(!opening);
            let mut start = std::time::Instant::now();
            state.values(opening, start);
            if let Some(interrupt) = case["interruptAt"].as_u64() {
                start += std::time::Duration::from_millis(interrupt);
                state.values(opening, start);
                opening = !opening;
            }
            for frame in case["samples"].as_array().unwrap() {
                let ms = frame["milliseconds"].as_u64().unwrap();
                let (width, opacity, animating) =
                    state.values(opening, start + std::time::Duration::from_millis(ms));
                assert!(
                    (f64::from(width) - frame["width"].as_f64().unwrap()).abs() < 0.02,
                    "{case}; {frame}; {width}"
                );
                assert!(
                    (f64::from(opacity) - frame["inputOpacity"].as_f64().unwrap()).abs() < 0.00002,
                    "{case}; {frame}; {opacity}"
                );
                assert!(
                    (f64::from(1. - opacity) - frame["buttonOpacity"].as_f64().unwrap()).abs()
                        < 0.00002
                );
                if ms == 350 {
                    assert!(!animating);
                }
            }
        }
    }
}
