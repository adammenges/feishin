use crate::{app::Feishin, styles};
use feishin_core::{
    i18n,
    model::{Item, ItemKind},
    styles::Target,
};
use gpui::{
    AnyElement, App, Context, Div, Entity, Render, Subscription, WeakEntity, Window, div, hsla,
    linear_color_stop, linear_gradient, prelude::*, px,
};
use gpui_component::{ActiveTheme, Icon, StyledExt};

pub struct Preview {
    pub view: WeakEntity<Feishin>,
    pub item: Item,
    pub count: usize,
    pub drag: bool,
    pub _subscription: Option<Subscription>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.view
            .upgrade()
            .map(|view| {
                view.read(cx)
                    .item_preview(&self.item, self.count, self.drag, cx)
            })
            .unwrap_or_else(|| div().into_any_element())
    }
}

pub fn stylesheet() -> String {
    let mut result = String::new();
    for (source, module) in [
        (
            include_str!("../assets/styles/context-preview.css"),
            "context-menu-preview",
        ),
        (
            include_str!("../assets/styles/drag-preview.css"),
            "drag-preview",
        ),
        (
            include_str!("../assets/styles/selection-dialog.css"),
            "selection-dialog",
        ),
    ] {
        let mut source = source
            .lines()
            .filter(|line| {
                // The native drag layer and passive overlay handle event routing.
                // The drag artwork's constant transform is applied at paint time.
                let native_position = module == "selection-dialog"
                    && ["position:", "bottom:", "left:", "z-index:"]
                        .iter()
                        .any(|property| line.trim_start().starts_with(property));
                let decoration = [
                    "pointer-events:",
                    "user-select:",
                    "transform",
                    "perspective:",
                    "transition:",
                ]
                .iter()
                .any(|property| line.trim_start().starts_with(property));
                !(native_position || decoration)
            })
            .collect::<Vec<_>>()
            .join("\n");
        for name in [
            "selection-indicator",
            "image-container",
            "image-overlay",
            "icon-container",
            "info-icon",
            "text-container",
            "container",
            "divider",
            "preview",
            "content",
            "image",
            "name",
            "count",
        ] {
            source = source.replace(&format!(".{name}"), &format!(".fs-{module}-module-{name}"));
        }
        result.push_str(&source);
        result.push('\n');
    }
    result.push_str(&crate::styles::icon_stylesheet());
    result
}

pub fn drag_constructor(
    cx: &Context<Feishin>,
) -> impl Fn(&crate::queue::Dragged, gpui::Point<gpui::Pixels>, &mut Window, &mut App) -> Entity<Preview>
+ 'static {
    let parent = cx.weak_entity();
    move |dragged, _, _, cx| {
        let view = parent.clone();
        let item = dragged.items.first().cloned().unwrap_or_else(|| Item {
            name: "Item".into(),
            ..Default::default()
        });
        let count = dragged.items.len();
        if let Some(parent) = view.upgrade() {
            parent.update(cx, |view, cx| view.fetch_item_cover(&item, cx));
        }
        drag_preview(view, item, count, cx)
    }
}

pub fn folder_drag_constructor(
    cx: &Context<Feishin>,
) -> impl Fn(
    &crate::playlists::FolderDrag,
    gpui::Point<gpui::Pixels>,
    &mut Window,
    &mut App,
) -> Entity<Preview>
+ 'static {
    let parent = cx.weak_entity();
    move |dragged, _, _, cx| {
        drag_preview(
            parent.clone(),
            Item {
                name: dragged.path.clone(),
                kind: ItemKind::Folder,
                ..Default::default()
            },
            1,
            cx,
        )
    }
}

fn drag_preview(
    view: WeakEntity<Feishin>,
    item: Item,
    count: usize,
    cx: &mut App,
) -> Entity<Preview> {
    cx.new(|cx| Preview {
        _subscription: view
            .upgrade()
            .map(|view| cx.observe(&view, |_, _, cx| cx.notify())),
        view,
        item,
        count,
        drag: true,
    })
}

impl Feishin {
    pub fn item_preview(&self, item: &Item, count: usize, drag: bool, cx: &App) -> AnyElement {
        let module = if drag {
            "drag-preview"
        } else {
            "context-menu-preview"
        };
        let style = |element: Div, part: &str, cx: &App| {
            styles::apply(
                element,
                "div",
                "",
                &[&format!("fs-{module}-module-{part}")],
                (!drag).then(|| {
                    Target::new(
                        "div",
                        "native-popup-menu",
                        &["fs-context-menu-module-content"],
                    )
                }),
                cx,
            )
            .when(
                drag && matches!(part, "image-container" | "icon-container"),
                |div| div.paint_scale(1.05),
            )
        };
        let art_size = if drag { 48. } else { 40. };
        let art = if !item.cover_id.is_empty()
            || self
                .artwork_path_at(item, feishin_core::images::Placement::Table)
                .is_some()
            || self
                .image_placeholders
                .borrow_mut()
                .get(item, self.state.settings.image_placeholder)
                .is_some()
        {
            style(
                div()
                    .debug_selector(move || {
                        if drag {
                            "drag-preview-art".into()
                        } else {
                            "context-preview-art".into()
                        }
                    })
                    .relative()
                    .size(px(art_size))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(self.image_content_in(
                        item,
                        feishin_core::images::Placement::Table,
                        px(0.),
                        Some((
                            Target::new(
                                "img",
                                "",
                                &[
                                    &format!("fs-{module}-module-image"),
                                    "fs-image-module-image",
                                ],
                            ),
                            vec![Target::new(
                                "div",
                                "",
                                &[&format!("fs-{module}-module-image-container")],
                            )],
                        )),
                        cx,
                    ))
                    .child(style(
                        div().absolute().inset_0().bg(linear_gradient(
                            135.,
                            linear_color_stop(hsla(0., 0., 1., 0.1), 0.),
                            linear_color_stop(hsla(0., 0., 0., 0.1), 1.),
                        )),
                        "image-overlay",
                        cx,
                    )),
                "image-container",
                cx,
            )
            .into_any_element()
        } else {
            let icon = match item.kind {
                ItemKind::Artist | ItemKind::AlbumArtist => "artist",
                ItemKind::Album => "album",
                ItemKind::Folder => "folder",
                ItemKind::Playlist => "playlist",
                ItemKind::Genre => "genre",
                ItemKind::Radio => "radio",
                ItemKind::Song => "song",
            };
            style(
                div()
                    .debug_selector(move || {
                        if drag {
                            "drag-preview-icon".into()
                        } else {
                            "context-preview-icon".into()
                        }
                    })
                    .size(px(art_size))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(linear_gradient(
                        135.,
                        linear_color_stop(cx.theme().secondary, 0.),
                        linear_color_stop(cx.theme().background, 1.),
                    ))
                    .child(styles::apply(
                        div().child(Icon::default().path(format!("native-icons/{icon}.svg"))),
                        "svg",
                        "",
                        &[if drag {
                            "fs-icon-module-size-xl"
                        } else {
                            "fs-icon-module-size-md"
                        }],
                        None,
                        cx,
                    )),
                "icon-container",
                cx,
            )
            .into_any_element()
        };
        style(
            div()
                .debug_selector(move || {
                    if drag {
                        "drag-preview".into()
                    } else {
                        "context-preview".into()
                    }
                })
                .w_full()
                .v_flex()
                .when(!drag, |div| {
                    div.child(style(
                        gpui::div().h(px(1.)).my_1().bg(cx.theme().border),
                        "divider",
                        cx,
                    ))
                })
                .child(style(
                    div().h_flex().items_center().p_2().child(style(
                        div()
                            .h_flex()
                            .when(i18n::rtl(&self.state.settings.language), |div| {
                                div.flex_row_reverse()
                            })
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(art)
                            .child(style(
                                div()
                                    .v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(style(
                                        div()
                                            .debug_selector(move || {
                                                if drag {
                                                    "drag-preview-name".into()
                                                } else {
                                                    "context-preview-name".into()
                                                }
                                            })
                                            .truncate()
                                            .font_semibold()
                                            .child(item.name.clone()),
                                        "name",
                                        cx,
                                    ))
                                    .when(count > 1, |div| {
                                        let more = i18n::translate_with(
                                            &self.state.settings.language,
                                            "common.itemsMore",
                                            &serde_json::json!({"count":count-1}),
                                        )
                                        .unwrap_or_else(|| format!("{} more", count - 1));
                                        div.child(style(
                                            gpui::div()
                                                .debug_selector(move || {
                                                    if drag {
                                                        "drag-preview-count".into()
                                                    } else {
                                                        "context-preview-count".into()
                                                    }
                                                })
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!("+{more}")),
                                            "count",
                                            cx,
                                        ))
                                    }),
                                "text-container",
                                cx,
                            )),
                        "content",
                        cx,
                    )),
                    "preview",
                    cx,
                )),
            "container",
            cx,
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feishin_core::{queue::Enqueue, storage::State};
    use gpui::{
        Entity, Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase,
        point,
    };
    use gpui_component::{Root, menu::DropdownMenu};
    use std::{cell::RefCell, rc::Rc};

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn preview_artwork_keeps_original_cover_fit_and_custom_css(cx: &mut TestAppContext) {
        struct Probe {
            view: Entity<Feishin>,
            item: Item,
            drag: bool,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div().w(px(300.)).child(
                    self.view
                        .read(cx)
                        .item_preview(&self.item, 1, self.drag, cx),
                )
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-preview-fit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("cover.png");
        image::RgbaImage::from_pixel(120, 40, image::Rgba([238, 51, 68, 255]))
            .save(&source)
            .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.native_image_aspect_ratio = true;
            state.settings.image_placeholder = feishin_core::images::Placeholder::Off;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let item = Item {
                id: "rectangular".into(),
                name: "Rectangular artwork".into(),
                cover_id: "cover".into(),
                ..Default::default()
            };
            app.update(cx, |view, _| {
                view.covers.insert(
                    view.cover_cache_key(&item, feishin_core::images::Placement::Table),
                    source.clone(),
                );
            });
            let probe = cx.new(|_| Probe {
                view: app.clone(),
                item,
                drag: false,
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        for drag in [false, true] {
            for contain in [false, true] {
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        view.state.settings.css.enabled = true;
                        view.save_custom_css(if contain {
                            ".fs-context-menu-preview-module-image, .fs-drag-preview-module-image { object-fit: contain; }"
                        } else { "" }.into(), window, cx);
                    });
                    probe.update(cx, |probe, cx| { probe.drag = drag; cx.notify(); });
                });
                cx.run_until_parked();
                let images = cx.update(|window, _| window.painted_images_for_test());
                let width = if drag { 48. * 1.05 } else { 40. };
                let height = if contain { width / 3. } else { width };
                assert!(
                    images
                        .iter()
                        .any(
                            |(bounds, _, _)| (f32::from(bounds.size.width) - width).abs() < 0.1
                                && (f32::from(bounds.size.height) - height).abs() < 0.1
                        ),
                    "drag={drag}, contain={contain}: {images:?}"
                );
                cx.update(|_, cx| assert!(cx.global::<styles::NativeStyles>().warnings.is_empty()));
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn drag_preview_layout_and_painted_art_match_original(cx: &mut TestAppContext) {
        struct Probe {
            view: Entity<Feishin>,
            item: Item,
            count: usize,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let view = self.view.read(cx);
                div()
                    .w(px(300. * view.state.settings.zoom))
                    .child(view.item_preview(&self.item, self.count, true, cx))
            }
        }
        cx.use_platform_text_system();
        cx.update(gpui_component::init);
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/drag-preview-layout-linux-reference.json")
        } else {
            include_str!("../assets/drag-preview-layout-darwin-reference.json")
        })
        .unwrap();
        let dir =
            std::env::temp_dir().join(format!("feishin-drag-layout-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|_| Probe {
                view: app.clone(),
                item: Item {
                    name: "A selected song".into(),
                    dominant_color: "#88eeaa".into(),
                    ..Default::default()
                },
                count: 1,
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let rem = case["rem"].as_f64().unwrap() as f32;
            let image = case["image"].as_bool().unwrap();
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.zoom = zoom;
                    view.state.settings.language =
                        if case["rtl"] == true { "ar" } else { "en" }.into();
                    view.state.settings.image_placeholder = if image {
                        feishin_core::images::Placeholder::DominantColor
                    } else {
                        feishin_core::images::Placeholder::Off
                    };
                    view.apply_theme(window, cx);
                    gpui_component::Theme::global_mut(cx).font_size = px(rem * zoom);
                    window.set_rem_size(px(rem * zoom));
                });
                probe.update(cx, |probe, cx| {
                    probe.count = case["count"].as_u64().unwrap() as usize;
                    cx.notify();
                });
            });
            cx.run_until_parked();
            let preview = cx.debug_bounds("drag-preview").unwrap();
            let elements = case["elements"].as_array().unwrap();
            let expected = |name: &str| {
                let element = elements
                    .iter()
                    .find(|element| element["name"] == name)
                    .unwrap();
                let values = element["bounds"].as_array().unwrap();
                gpui::bounds(
                    preview.origin
                        + point(
                            px(values[0].as_f64().unwrap() as f32 * zoom),
                            px(values[1].as_f64().unwrap() as f32 * zoom),
                        ),
                    gpui::size(
                        px(values[2].as_f64().unwrap() as f32 * zoom),
                        px(values[3].as_f64().unwrap() as f32 * zoom),
                    ),
                )
            };
            let matches = |actual: gpui::Bounds<gpui::Pixels>,
                           expected: gpui::Bounds<gpui::Pixels>| {
                (actual.left() - expected.left()).abs() <= px(1.)
                    && (actual.top() - expected.top()).abs() <= px(1.)
                    && (actual.size.width - expected.size.width).abs() <= px(1.)
                    && (actual.size.height - expected.size.height).abs() <= px(1.)
            };
            for name in ["preview", "name", "count"] {
                if name == "count" && case["count"] == 1 {
                    continue;
                }
                let selector = if name == "preview" {
                    "drag-preview".to_owned()
                } else {
                    format!("drag-preview-{name}")
                };
                let actual = cx.debug_bounds(&selector).unwrap();
                assert!(
                    matches(actual, expected(name)),
                    "{name}: {actual:?} != {:?}: {case}",
                    expected(name)
                );
            }
            let art = expected(if image {
                "image-container"
            } else {
                "icon-container"
            });
            let painted = cx.update(|window, _| window.painted_images_for_test());
            assert!(
                painted.iter().any(|(bounds, _, _)| matches(*bounds, art)),
                "Painted artwork: {painted:?}, expected {art:?}: {case}"
            );
            let layout = cx
                .debug_bounds(if image {
                    "drag-preview-art"
                } else {
                    "drag-preview-icon"
                })
                .unwrap();
            assert_eq!(layout.size, gpui::size(px(48. * zoom), px(48. * zoom)));
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui::test]
    fn folder_context_download_opens_a_cancelable_save_dialog(cx: &mut TestAppContext) {
        use feishin_core::{
            api::Api,
            model::{Server, ServerType, Session},
        };
        struct Probe {
            view: Entity<Feishin>,
            folder: Item,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.view.update(cx, |view, cx| {
                    div().p_4().child(
                        styles::button("folder-menu", cx)
                            .debug_selector(|| "folder-menu".into())
                            .label("More")
                            .dropdown_menu(view.item_menu_builder(
                                &self.folder,
                                vec![self.folder.clone()],
                                false,
                                cx,
                            )),
                    )
                })
            }
        }
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-folder-download-{}", uuid::Uuid::new_v4()));
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Fixture".into(),
            url: "http://127.0.0.1:1".into(),
            username: "fixture".into(),
            kind: ServerType::Subsonic,
            legacy_auth: false,
            options: Default::default(),
        };
        let folder = Item {
            id: "folder".into(),
            name: "Music folder".into(),
            kind: ItemKind::Folder,
            server_id: server.id,
            ..Default::default()
        };
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |view, _| {
                view.sessions
                    .insert(server.id, Api::new(server, Session::default()).unwrap());
            });
            captured.replace(Some(app.clone()));
            let probe = cx.new(|_| Probe { view: app, folder });
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        let trigger = cx.debug_bounds("folder-menu").unwrap().center();
        cx.simulate_click(trigger, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.value().or_else(|| node.label()) == Some("Download"))
        }));
        let download = cx.debug_bounds("popup-menu-item-2").unwrap().center();
        cx.simulate_click(download, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| serde_json::to_value(&view.state.queue)
                .unwrap()),
            queue
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
    fn library_selection_dialog_floats_clears_and_opens_duplicate_safe_actions(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-selection-dialog-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = crate::app::Route::Library(ItemKind::Song);
                view.items = vec![
                    Item {
                        id: "duplicate".into(),
                        name: "First selected song".into(),
                        ..Default::default()
                    },
                    Item {
                        id: "duplicate".into(),
                        name: "Second selected song".into(),
                        ..Default::default()
                    },
                ];
                view.loading = false;
                view.selected.extend([0, 1]);
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.route.clone()),
            crate::app::Route::Library(ItemKind::Song)
        );
        let library = cx.debug_bounds("library-drop").unwrap();
        let dialog = cx.debug_bounds("library-selection-dialog").unwrap();
        assert!(dialog.size.width >= px(320.), "{dialog:?}");
        assert!(
            (dialog.center().x - library.center().x).abs() <= px(1.),
            "{dialog:?} / {library:?}"
        );
        assert!(
            (library.bottom() - dialog.bottom() - px(16.)).abs() <= px(1.),
            "{dialog:?} / {library:?}"
        );
        let blur = cx.update(|window, _| window.painted_backdrops_for_test());
        let filter = cx.update(|_, cx| {
            cx.global::<styles::NativeStyles>().style(
                Target::new(
                    "div",
                    "library-selection-dialog",
                    &["fs-selection-dialog-module-selection-indicator"],
                ),
                None,
                16.,
            )
        });
        assert_eq!(filter.backdrop_blur, Some(px(12.)), "{filter:?}");
        assert!(
            blur.iter()
                .any(|(bounds, _, _, sigma, _)| *bounds == dialog && *sigma == px(12.)),
            "{blur:?}, dialog={dialog:?}, library={library:?}, viewport={:?}",
            cx.update(|window, _| window.viewport_size())
        );
        let clear = cx.debug_bounds("selected-clear").unwrap().center();
        cx.simulate_click(clear, Modifiers::none());
        cx.run_until_parked();
        assert!(!app.read_with(cx, |view, _| view.has_library_selection()));
        assert_eq!(cx.debug_bounds("library-drop").unwrap(), library);
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.selected.extend([0, 1]);
                cx.notify();
            })
        });
        cx.run_until_parked();
        let more = cx.debug_bounds("selected-more").unwrap().center();
        cx.simulate_click(more, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("context-preview-name").is_some());
        assert!(cx.debug_bounds("context-preview-count").is_some());
        let info = cx.debug_bounds("popup-menu-item-0").unwrap().center();
        cx.simulate_click(info, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.route, crate::app::Route::Info);
            assert_eq!(
                view.info_items
                    .iter()
                    .map(|item| (item.id.as_str(), item.name.as_str()))
                    .collect::<Vec<_>>(),
                vec![
                    ("duplicate", "First selected song"),
                    ("duplicate", "Second selected song")
                ]
            );
        });
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui::test]
    fn drag_preview_keeps_first_item_translates_count_and_drops_duplicate_keys(
        cx: &mut TestAppContext,
    ) {
        struct Probe {
            view: Entity<Feishin>,
            preview: Rc<RefCell<Option<Entity<Preview>>>>,
            dropped: Rc<RefCell<Option<crate::queue::Dragged>>>,
            folder: Rc<RefCell<Option<String>>>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.view.update(cx, |view, cx| {
                    let preview = self.preview.clone();
                    let constructor = drag_constructor(cx);
                    let dropped = self.dropped.clone();
                    let folder = self.folder.clone();
                    div()
                        .h_flex()
                        .gap_4()
                        .p_4()
                        .child(
                            div()
                                .id("drag-source")
                                .debug_selector(|| "drag-source".into())
                                .size(px(100.))
                                .child("Songs")
                                .on_drag(
                                    view.drag_queue_entry(view.state.queue.order[0], false),
                                    move |data, offset, window, cx| {
                                        let entity = constructor(data, offset, window, cx);
                                        preview.replace(Some(entity.clone()));
                                        entity
                                    },
                                ),
                        )
                        .child(
                            div()
                                .id("folder-source")
                                .debug_selector(|| "folder-source".into())
                                .size(px(100.))
                                .child("Folder")
                                .on_drag(
                                    crate::playlists::FolderDrag {
                                        path: "Music/Favorites".into(),
                                        shared: false,
                                    },
                                    folder_drag_constructor(cx),
                                ),
                        )
                        .child(
                            div()
                                .id("drag-target")
                                .debug_selector(|| "drag-target".into())
                                .size(px(100.))
                                .child("Drop")
                                .on_drop(move |data: &crate::queue::Dragged, _, _| {
                                    dropped.replace(Some(data.clone()));
                                })
                                .on_drop(move |data: &crate::playlists::FolderDrag, _, _| {
                                    folder.replace(Some(data.path.clone()));
                                }),
                        )
                })
            }
        }
        cx.use_platform_text_system();
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-drag-preview-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let preview = Rc::new(RefCell::new(None));
        let dropped = Rc::new(RefCell::new(None));
        let folder = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.image_placeholder = feishin_core::images::Placeholder::DominantColor;
            state.queue.add(
                vec![
                    Item {
                        id: "duplicate".into(),
                        name: "First selected song".into(),
                        dominant_color: "#102030".into(),
                        ..Default::default()
                    },
                    Item {
                        id: "duplicate".into(),
                        name: "Second selected song".into(),
                        ..Default::default()
                    },
                ],
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |view, _| {
                view.queue_selected
                    .extend(view.state.queue.order.iter().copied())
            });
            captured.replace(Some(app.clone()));
            let probe = cx.new(|_| Probe {
                view: app,
                preview: preview.clone(),
                dropped: dropped.clone(),
                folder: folder.clone(),
            });
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        let keys = app.read_with(cx, |view, _| view.state.queue.order.clone());
        cx.run_until_parked();
        let source = cx.debug_bounds("drag-source").unwrap().center();
        let target = cx.debug_bounds("drag-target").unwrap().center();
        cx.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            source + point(px(24.), px(0.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.update(|_, cx| cx.has_active_drag()));
        let bounds = cx.debug_bounds("drag-preview").unwrap();
        assert_eq!(bounds.size.width, px(300.));
        assert!(bounds.size.height >= px(80.), "{bounds:?}");
        assert_eq!(
            cx.debug_bounds("drag-preview-art").unwrap().size,
            gpui::size(px(48.), px(48.))
        );
        let preview = preview.borrow().clone().unwrap();
        assert_eq!(
            preview.read_with(cx, |view, _| (view.item.name.clone(), view.count)),
            ("First selected song".into(), 2)
        );
        assert!(cx.debug_bounds("drag-preview-count").is_some());
        let count_bounds = cx.debug_bounds("drag-preview-count").unwrap();
        let glyphs = |cx: &mut gpui::VisualTestContext| {
            cx.update(|window, _| {
                window
                    .painted_glyphs_for_test()
                    .into_iter()
                    .filter(|(bounds, _, _)| bounds.intersects(&count_bounds))
                    .collect::<Vec<_>>()
            })
        };
        let english_glyphs = glyphs(cx);
        assert!(!english_glyphs.is_empty());
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.language = "fr".into();
                view.apply_theme(window, cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_ne!(glyphs(cx), english_glyphs);
        cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(!cx.update(|_, cx| cx.has_active_drag()));
        let dropped = dropped.borrow_mut().take().unwrap();
        assert_eq!(dropped.keys, keys);
        assert_eq!(
            dropped
                .items
                .iter()
                .map(|item| (item.id.as_str(), item.name.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("duplicate", "First selected song"),
                ("duplicate", "Second selected song")
            ]
        );

        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.image_placeholder = feishin_core::images::Placeholder::Off;
                cx.notify();
            })
        });
        let source = cx.debug_bounds("drag-source").unwrap().center();
        cx.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            source + point(px(24.), px(0.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("drag-preview-icon").unwrap().size,
            gpui::size(px(48.), px(48.))
        );
        cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();

        let source = cx.debug_bounds("folder-source").unwrap().center();
        cx.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            source + point(px(24.), px(0.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("drag-preview").unwrap().size.width,
            px(300.)
        );
        assert_eq!(
            cx.debug_bounds("drag-preview-icon").unwrap().size,
            gpui::size(px(48.), px(48.))
        );
        cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(folder.borrow().as_deref(), Some("Music/Favorites"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui::test]
    fn context_preview_preserves_duplicate_selection_and_stays_fixed_while_actions_scroll(
        cx: &mut TestAppContext,
    ) {
        struct Probe {
            view: Entity<Feishin>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.view.update(cx, |view, cx| {
                    div()
                        .p_4()
                        .child(
                            styles::button("preview-probe", cx)
                                .debug_selector(|| "preview-probe".into())
                                .label("More")
                                .dropdown_menu(view.queue_menu_builder(
                                    view.state.queue.current().unwrap(),
                                    false,
                                    cx,
                                )),
                        )
                        .into_any_element()
                })
            }
        }
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-preview-ui-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.queue.add(
                vec![
                    Item {
                        id: "duplicate".into(),
                        name: "First selected song".into(),
                        ..Default::default()
                    },
                    Item {
                        id: "duplicate".into(),
                        name: "Second selected song".into(),
                        ..Default::default()
                    },
                ],
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |view, _| {
                view.queue_selected
                    .extend(view.state.queue.order.iter().copied())
            });
            captured.replace(Some(app.clone()));
            let probe = cx.new(|_| Probe { view: app });
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let trigger = cx.debug_bounds("preview-probe").unwrap().center();
        cx.simulate_click(trigger, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("context-preview-name").is_some());
        assert!(cx.debug_bounds("context-preview-count").is_some());
        let before = cx.debug_bounds("context-preview").unwrap();
        let name_offset =
            cx.debug_bounds("context-preview-name").unwrap().origin.y - before.origin.y;
        let first = cx.debug_bounds("popup-menu-item-0").unwrap();
        assert!(before.origin.y > first.origin.y);
        cx.simulate_event(ScrollWheelEvent {
            position: first.center(),
            delta: ScrollDelta::Lines(point(0., -30.)),
            touch_phase: TouchPhase::Moved,
            modifiers: Modifiers::none(),
        });
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("context-preview").unwrap(), before);
        assert!(
            cx.debug_bounds("popup-menu-item-0")
                .is_none_or(|bounds| bounds.origin.y < first.origin.y)
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        cx.update(|_, cx| app.update(cx, |view, _| view.queue_selected.clear()));
        let trigger = cx.debug_bounds("preview-probe").unwrap().center();
        cx.simulate_click(trigger, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("context-preview-name").is_some());
        // GPUI retains historic debug selectors. Removing the count centers the
        // shorter text column lower alongside the fixed-size thumbnail.
        let preview = cx.debug_bounds("context-preview").unwrap();
        let name = cx.debug_bounds("context-preview-name").unwrap();
        assert!(name.origin.y - preview.origin.y > name_offset);
        std::fs::remove_dir_all(dir).ok();
    }
}
