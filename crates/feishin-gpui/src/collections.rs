use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{collections::Collection, filters::View, model::ItemKind};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, Selectable, Sizable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::{InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    popover::Popover,
};

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "collection_sidebar_tests.rs"]
mod sidebar_tests;

pub(crate) struct CollectionRename {
    pub id: String,
    pub input: Entity<InputState>,
    _subscription: Subscription,
}

impl Feishin {
    fn rename_sidebar_collection(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = self
            .state
            .settings
            .collections
            .iter()
            .find(|collection| collection.id == id)
        else {
            return;
        };
        let input = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .default_value(collection.name.clone())
                .aria_label("Name")
                .max_length(128)
        });
        let subscription = cx.subscribe_in(&input, window, |view, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                view.save_sidebar_collection_name(cx);
            }
        });
        self.collection_rename = Some(CollectionRename {
            id: id.into(),
            input: input.clone(),
            _subscription: subscription,
        });
        window.defer(cx, move |window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx))
        });
        cx.notify();
    }
    fn save_sidebar_collection_name(&mut self, cx: &mut Context<Self>) {
        let Some(rename) = &self.collection_rename else {
            return;
        };
        let name = rename.input.read(cx).value().trim().to_owned();
        if name.is_empty() || name.encode_utf16().count() > 256 {
            return;
        }
        let id = rename.id.clone();
        if let Some(collection) = self
            .state
            .settings
            .collections
            .iter_mut()
            .find(|collection| collection.id == id)
        {
            collection.name.clone_from(&name);
            if let Route::Collection(current) = &mut self.route
                && current.id == id
            {
                current.name.clone_from(&name);
            }
            for visit in self
                .history
                .iter_mut()
                .chain(self.forward_history.iter_mut())
            {
                if let Route::Collection(current) = &mut visit.route
                    && current.id == id
                {
                    current.name.clone_from(&name);
                }
            }
            self.save();
        }
        self.collection_rename = None;
        cx.notify();
    }
    fn sidebar_collection_rename_view(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(rename) = &self.collection_rename else {
            return div().into_any_element();
        };
        crate::speaker_picker::popup_surface(
            div()
                .id("sidebar-collection-rename")
                .debug_selector(|| "sidebar-collection-rename".into())
                .role(Role::Dialog)
                .aria_label(self.tr("common.rename"))
                .on_action(|_: &gpui_component::input::Enter, _, _| {})
                .w(rems(17.5)),
            "sidebar-collection-rename",
            window,
            self.state.settings.zoom,
            cx,
        )
        .child(
            div()
                .v_flex()
                .p(rems(0.25))
                .gap(rems(0.75))
                .child(
                    div()
                        .debug_selector(|| "sidebar-collection-name".into())
                        .child(Styled::h(
                            crate::styles::input(&rename.input, cx),
                            rems(2.25),
                        )),
                )
                .child(
                    div()
                        .h_flex()
                        .justify_end()
                        .gap(rems(0.25))
                        .when(crate::appearance::rtl(cx), |div| {
                            div.flex_row_reverse().justify_start()
                        })
                        .child(
                            crate::styles::button("sidebar-collection-rename-cancel", cx)
                                .debug_selector(|| "sidebar-collection-rename-cancel".into())
                                .ghost()
                                .label(self.tr("common.cancel"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.collection_rename = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            crate::styles::button("sidebar-collection-rename-save", cx)
                                .debug_selector(|| "sidebar-collection-rename-save".into())
                                .primary()
                                .label(self.tr("common.save"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.save_sidebar_collection_name(cx)
                                })),
                        ),
                ),
        )
        .into_any_element()
    }
    pub fn collection_kind(&self) -> Option<ItemKind> {
        let kind = match &self.route {
            Route::Library(kind) | Route::Favorites(kind) => *kind,
            Route::Collection(collection) => collection.kind,
            Route::Detail(item) if item.kind == ItemKind::Genre => {
                if self.state.settings.genre_albums {
                    ItemKind::Album
                } else {
                    ItemKind::Song
                }
            }
            _ => return None,
        };
        matches!(kind, ItemKind::Album | ItemKind::Song).then_some(kind)
    }
    pub fn edit_collection(
        &mut self,
        id: Option<&str>,
        current: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = id
            .and_then(|id| {
                self.state
                    .settings
                    .collections
                    .iter()
                    .find(|collection| collection.id == id)
            })
            .cloned();
        let draft = if current {
            let Some(kind) = self.collection_kind() else {
                return;
            };
            let mut filters = self.active_filters.clone();
            if matches!(self.route, Route::Favorites(_)) {
                filters.favorite = Some(true);
            }
            if let Route::Detail(item) = &self.route
                && item.kind == ItemKind::Genre
            {
                filters.genre_ids = vec![item.id.clone()];
                filters.genre_all = false;
            }
            Collection {
                id: uuid::Uuid::new_v4().to_string(),
                name: String::new(),
                kind,
                view: View {
                    filters,
                    text: self.local_filter.clone(),
                    sort: self.sort,
                    descending: self.descending,
                    artist_role: self.artist_role.clone(),
                },
            }
        } else if let Some(existing) = existing {
            existing
        } else {
            return;
        };
        self.form_name.update(cx, |input, cx| {
            input.set_value(draft.name.clone(), window, cx)
        });
        self.collection_draft = Some(draft);
        self.navigate(Route::CollectionEditor, cx);
    }
    pub fn save_collection(&mut self, overwrite: Option<String>, cx: &mut Context<Self>) {
        let Some(mut draft) = self.collection_draft.clone() else {
            return;
        };
        draft.name = self.form_name.read(cx).value().trim().to_owned();
        if let Some(id) = overwrite {
            let Some(existing) = self
                .state
                .settings
                .collections
                .iter()
                .find(|collection| collection.id == id && collection.kind == draft.kind)
            else {
                return;
            };
            draft.id = id;
            draft.name = existing.name.clone();
        }
        let mut collections = self.state.settings.collections.clone();
        if let Some(index) = collections
            .iter()
            .position(|collection| collection.id == draft.id)
        {
            collections[index] = draft.clone();
        } else {
            collections.push(draft.clone());
        }
        if let Err(error) = feishin_core::collections::validate(&collections) {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        let previous = self
            .state
            .settings
            .collections
            .iter()
            .find(|collection| collection.id == draft.id);
        if previous.is_some_and(|previous| previous.view != draft.view) {
            self.state
                .settings
                .views
                .retain(|key, _| !key.ends_with(&format!(":collection:{}", draft.id)));
        }
        self.state.settings.collections = collections;
        for route in self
            .history
            .iter_mut()
            .chain(self.forward_history.iter_mut())
        {
            if matches!(&route.route, Route::Collection(collection) if collection.id == draft.id) {
                route.route = Route::Collection(Box::new(draft.clone()));
            }
        }
        self.save();
        self.back(cx);
    }
    pub fn remove_collection(&mut self, id: &str, cx: &mut Context<Self>) {
        if self
            .collection_rename
            .as_ref()
            .is_some_and(|rename| rename.id == id)
        {
            self.collection_rename = None;
        }
        self.state
            .settings
            .collections
            .retain(|collection| collection.id != id);
        // Removing a bookmark leaves its current and historical filtered pages intact.
        self.save();
        cx.notify();
    }
    pub fn collapsed_sidebar_collections(&self, cx: &mut Context<Self>) -> AnyElement {
        let collections = self.state.settings.collections.clone();
        let current_kind = match &self.route {
            Route::Library(kind) => Some(*kind),
            Route::Collection(collection) => Some(collection.kind),
            _ => None,
        };
        let view = cx.weak_entity();
        self.sidebar_navigation_control("Collections", false, cx)
            .debug_selector(|| "sidebar-nav-Collections".into())
            .dropdown_menu(move |mut menu, window, _| {
                menu = menu.max_h(window.viewport_size().height / 2.);
                for collection in &collections {
                    let collection = collection.clone();
                    let view = view.clone();
                    let name = crate::sidebar::navigation_icon(
                        if collection.kind == ItemKind::Album {
                            "Albums"
                        } else {
                            "Tracks"
                        },
                        current_kind == Some(collection.kind),
                    )
                    .unwrap();
                    let icon =
                        gpui_component::Icon::default().path(format!("native-icons/{name}.svg"));
                    menu = menu.item(
                        PopupMenuItem::new(collection.name.clone())
                            .icon(icon)
                            .on_click(move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.navigate(
                                        Route::Collection(Box::new(collection.clone())),
                                        cx,
                                    )
                                })
                                .ok();
                            }),
                    );
                }
                menu
            })
            .side(
                if crate::appearance::rtl(cx) {
                    Corner::BottomRight
                } else {
                    Corner::BottomLeft
                },
                px(0.),
                px(5. * self.state.settings.zoom),
            )
            .into_any_element()
    }
    fn collection_selected(&self, collection: &Collection) -> bool {
        let kind = match &self.route {
            Route::Collection(current) => current.kind,
            Route::Library(kind) => *kind,
            _ => return false,
        };
        let view = &collection.view;
        kind == collection.kind
            && view.filters == self.active_filters
            && view.text == self.local_filter
            && view.sort == self.sort
            && view.descending == self.descending
            && view.artist_role == self.artist_role
    }
    pub fn sidebar_collections(&self, cx: &mut Context<Self>) -> AnyElement {
        let rem = cx.theme().font_size;
        let zoom = self.state.settings.zoom;
        let rtl = crate::appearance::rtl(cx);
        let background = cx.theme().background.to_rgb();
        let factor = if cx.theme().is_dark() { 0.9 } else { 0.95 };
        let mix = if cx.theme().is_dark() { 0.1 } else { 0. };
        let hover: Hsla = Rgba {
            r: background.r * factor + mix,
            g: background.g * factor + mix,
            b: background.b * factor + mix,
            a: background.a,
        }
        .into();
        div()
            .v_flex()
            .flex_shrink_0()
            .pb(rem * 0.75)
            .children(self.state.settings.collections.iter().map(|collection| {
                let selected = self.collection_selected(collection);
                let foreground = if selected {
                    cx.theme().primary
                } else {
                    cx.theme().foreground
                };
                let open = collection.clone();
                let click = collection.clone();
                let rename = collection.id.clone();
                let remove = collection.id.clone();
                let view = cx.weak_entity();
                let content_view = cx.weak_entity();
                let rename_label = self.tr("common.rename");
                let remove_label = self.tr("common.delete");
                let id = collection.id.clone();
                let group = SharedString::from(format!("sidebar-collection-{id}"));
                let icon = crate::sidebar::navigation_icon(
                    if collection.kind == ItemKind::Album {
                        "Albums"
                    } else {
                        "Tracks"
                    },
                    selected,
                )
                .unwrap();
                let row = div()
                    .id(group.clone())
                    .group(group.clone())
                    .debug_selector({
                        let id = id.clone();
                        move || format!("sidebar-collection-{id}")
                    })
                    .h_flex()
                    .w_full()
                    .min_w_0()
                    .h(rem * 2.25 + px(2. * zoom))
                    .flex_shrink_0()
                    .gap(rem * 0.25)
                    .px(rem * 0.75)
                    .py(rem * 0.25)
                    .border(px(zoom))
                    .border_color(transparent_black())
                    .rounded(cx.theme().radius)
                    .when(selected, |div| div.bg(cx.theme().secondary))
                    .hover(move |style| style.bg(hover))
                    .when(rtl, |div| div.flex_row_reverse())
                    // The surrounding controlled popover opens only from Rename.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.navigate(Route::Collection(Box::new(click.clone())), cx)
                    }))
                    .child(
                        Button::new(SharedString::from(format!("collection-{id}")))
                            .custom(ButtonCustomVariant::new(cx).foreground(foreground))
                            .label_full_width()
                            .aria_label(collection.name.clone())
                            .selected(selected)
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .p_0()
                            .border_0()
                            .child(
                                div()
                                    .h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .gap(rem * 0.75)
                                    .text_color(foreground)
                                    .when(rtl, |div| div.flex_row_reverse())
                                    .child(
                                        div()
                                            .size(rem)
                                            .flex_shrink_0()
                                            .debug_selector({
                                                let id = id.clone();
                                                move || format!("sidebar-collection-icon-{id}")
                                            })
                                            .child(
                                                Icon::default()
                                                    .path(format!("native-icons/{icon}.svg"))
                                                    .with_size(rem),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .text_size(rem * 0.875)
                                            .line_height(rem * 1.125)
                                            .font_weight(FontWeight::MEDIUM)
                                            .truncate()
                                            .debug_selector({
                                                let id = id.clone();
                                                move || format!("sidebar-collection-name-{id}")
                                            })
                                            .child(collection.name.clone()),
                                    ),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.navigate(Route::Collection(Box::new(open.clone())), cx);
                                cx.stop_propagation();
                            })),
                    )
                    .child(
                        div()
                            .opacity(0.)
                            .group_hover(group, |style| style.opacity(1.))
                            .child(
                                Button::new(SharedString::from(format!("collection-menu-{id}")))
                                    .debug_selector({
                                        let id = id.clone();
                                        move || format!("sidebar-collection-menu-{id}")
                                    })
                                    .action_icon(zoom)
                                    .custom(ButtonCustomVariant::new(cx))
                                    .size(rem * 1.75)
                                    .p_0()
                                    .border(px(zoom))
                                    .border_color(transparent_black())
                                    .icon(
                                        Icon::default()
                                            .path("native-icons/more.svg")
                                            .rotate(Radians(std::f32::consts::FRAC_PI_2))
                                            .with_size(rem * 0.6875),
                                    )
                                    .tooltip(self.tr("common.menu"))
                                    .on_click(|_, _, cx| cx.stop_propagation())
                                    .dropdown_menu(move |menu, _, _| {
                                        let rename_view = view.clone();
                                        let remove_view = view.clone();
                                        let rename = rename.clone();
                                        let remove = remove.clone();
                                        menu.item(
                                            PopupMenuItem::new(rename_label.clone()).on_click(
                                                move |_, window, cx| {
                                                    let view = rename_view.clone();
                                                    let id = rename.clone();
                                                    window.defer(cx, move |window, cx| {
                                                        view.update(cx, |view, cx| {
                                                            view.rename_sidebar_collection(
                                                                &id, window, cx,
                                                            )
                                                        })
                                                        .ok();
                                                    });
                                                },
                                            ),
                                        )
                                        .item(
                                            PopupMenuItem::new(remove_label.clone()).on_click(
                                                move |_, _, cx| {
                                                    remove_view
                                                        .update(cx, |view, cx| {
                                                            view.remove_collection(&remove, cx)
                                                        })
                                                        .ok();
                                                },
                                            ),
                                        )
                                    })
                                    .side(
                                        if rtl {
                                            Corner::TopRight
                                        } else {
                                            Corner::TopLeft
                                        },
                                        px(10. * zoom),
                                        px(5. * zoom),
                                    ),
                            ),
                    );
                let renaming = self
                    .collection_rename
                    .as_ref()
                    .filter(|rename| rename.id == id);
                let mut trigger_style = StyleRefinement::default();
                trigger_style.size.width = Some(relative(1.).into());
                Popover::new(SharedString::from(format!(
                    "sidebar-collection-rename-{id}"
                )))
                .trigger_style(trigger_style)
                .trigger_element(row)
                .open(renaming.is_some())
                .when_some(renaming, |popover, rename| {
                    popover.track_focus(&rename.input.read(cx).focus_handle(cx))
                })
                .side(if rtl {
                    Corner::TopRight
                } else {
                    Corner::TopLeft
                })
                .offset(px(10. * zoom))
                .viewport_margin(px(5. * zoom))
                .appearance(false)
                .on_open_change(cx.listener(|view, open: &bool, _, cx| {
                    if !*open {
                        view.collection_rename = None;
                        cx.notify();
                    }
                }))
                .content(move |_, window, cx| {
                    content_view
                        .update(cx, |view, cx| {
                            view.sidebar_collection_rename_view(window, cx)
                        })
                        .unwrap_or_else(|_| div().into_any_element())
                })
            }))
            .into_any_element()
    }
    pub fn collections_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div().id("collections").debug_selector(|| "collections".into()).native_vertical_scroll().h_full().v_flex().gap_3()
            .when(self.state.settings.collections.is_empty(), |div| div.child(crate::appearance::text("Save the filters and sort order from an album or song library to create a collection.", cx)))
            .children(self.state.settings.collections.iter().enumerate().map(|(index, collection)| {
                let open = collection.clone(); let rename = collection.id.clone(); let remove = collection.id.clone();
                div().h_flex().when(crate::appearance::rtl(cx), |div| div.flex_row_reverse()).gap_3().child(div().flex_1().child(format!("{} ({})", collection.name, self.tr(&Route::Library(collection.kind).title()))))
                    .child(crate::styles::button(("collection-open",index), cx).label(crate::appearance::text("Open", cx)).on_click(cx.listener(move |this, _, _, cx| this.navigate(Route::Collection(Box::new(open.clone())),cx))))
                    .child(crate::styles::button(("collection-rename",index), cx).label(crate::appearance::text("Rename", cx)).on_click(cx.listener(move |this, _, window, cx| this.edit_collection(Some(&rename),false,window,cx))))
                    .child(crate::styles::button(("collection-remove",index), cx).label(crate::appearance::text("Remove", cx)).on_click(cx.listener(move |this, _, _, cx| this.remove_collection(&remove,cx))))
            })).into_any_element()
    }
    pub fn collection_editor_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(draft) = &self.collection_draft else {
            return div().into_any_element();
        };
        div()
            .id("collection-editor")
            .debug_selector(|| "collection-editor".into())
            .native_vertical_scroll()
            .h_full()
            .v_flex()
            .gap_3()
            .max_w(px(720.))
            .child(crate::styles::input(&self.form_name, cx))
            .child(div().child(format!(
                "{} - {} - {}",
                self.tr(&Route::Library(draft.kind).title()),
                self.tr(draft.view.sort.label()),
                self.tr(if draft.view.descending {
                    "Descending"
                } else {
                    "Ascending"
                })
            )))
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .child(
                        crate::styles::button("collection-save", cx)
                            .label(crate::appearance::text("Save", cx))
                            .on_click(cx.listener(|this, _, _, cx| this.save_collection(None, cx))),
                    )
                    .child(
                        crate::styles::button("collection-cancel", cx)
                            .ghost()
                            .label(crate::appearance::text("Cancel", cx))
                            .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                    ),
            )
            .children(
                self.state
                    .settings
                    .collections
                    .iter()
                    .filter(|collection| collection.kind == draft.kind && collection.id != draft.id)
                    .enumerate()
                    .map(|(index, collection)| {
                        let id = collection.id.clone();
                        crate::styles::button(("collection-overwrite", index), cx)
                            .label(format!("Overwrite {}", collection.name))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.save_collection(Some(id.clone()), cx)
                            }))
                    }),
            )
            .into_any_element()
    }
}
