use crate::app::{Feishin, Route};
use feishin_core::model::{Item, ItemKind};
use gpui::{prelude::*, *};

impl Feishin {
    pub(crate) fn joined_artists(
        &self,
        item: &Item,
        album: bool,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.artist_text(
            feishin_core::artists::joined(
                if album {
                    &item.album_artist
                } else {
                    &item.artist
                },
                &feishin_core::artists::links(item, album),
                false,
            ),
            item.server_id,
            id,
            cx,
        )
    }
    pub(crate) fn artist_names(
        &self,
        item: &Item,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.named_artists(
            feishin_core::artists::links(item, false),
            item.server_id,
            "  •  ",
            id,
            cx,
        )
    }
    pub(crate) fn named_artists(
        &self,
        artists: Vec<(String, String)>,
        server: uuid::Uuid,
        separator: &str,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.artist_text(
            feishin_core::artists::listed(artists, separator),
            server,
            id,
            cx,
        )
    }
    pub(crate) fn artist_text(
        &self,
        parts: Vec<feishin_core::artists::Part>,
        server: uuid::Uuid,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.linked_metadata(parts, server, ItemKind::AlbumArtist, id, cx)
    }
    pub(crate) fn linked_metadata(
        &self,
        parts: Vec<feishin_core::artists::Part>,
        server: uuid::Uuid,
        kind: ItemKind,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut text = String::new();
        let mut links = Vec::new();
        for part in parts {
            let start = text.len();
            text.push_str(&part.text);
            if let Some(id) = part.id {
                links.push((start..text.len(), id, part.text));
            }
        }
        let text = StyledText::new(text);
        let layout = text.layout().clone();
        let transform = std::rc::Rc::new(std::cell::Cell::new(None));
        let capture = transform.clone();
        let compact_player = id == "playerbar-compact-artists";
        div()
            .id(id)
            .debug_selector(move || id.into())
            .min_w_0()
            .max_w_full()
            .child(text)
            .when(!links.is_empty(), |div| {
                div.relative()
                    .cursor_pointer()
                    .child(
                        canvas(
                            {
                                let links = links.clone();
                                let layout = layout.clone();
                                move |bounds, window, cx| {
                                    let align = window.text_style().text_align;
                                    capture.set(Some((
                                        bounds,
                                        window.transform_element_bounds(bounds),
                                        align,
                                    )));
                                    layout.link_controls(
                                        links.iter().map(|(range, _, label)| {
                                            (range.clone(), SharedString::from(label.clone()))
                                        }),
                                        align,
                                        window,
                                        cx,
                                    )
                                }
                            },
                            |_, controls, window, cx| {
                                for mut control in controls {
                                    control.paint(window, cx)
                                }
                            },
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, event: &MouseUpEvent, _, cx| {
                            let Some((bounds, visual, align)) = transform.get() else {
                                return;
                            };
                            let scale = visual.size.width / bounds.size.width;
                            let position = if scale.is_finite() && scale > 0. {
                                bounds.origin + (event.position - visual.origin) / scale
                            } else {
                                event.position
                            };
                            if let Some((_, id, name)) = links.iter().find(|(range, _, _)| {
                                layout
                                    .range_bounds(range.clone(), align)
                                    .iter()
                                    .any(|bounds| bounds.contains(&position))
                            }) {
                                view.navigate_playerbar(
                                    Route::Detail(Box::new(Item {
                                        id: id.clone(),
                                        name: name.clone(),
                                        server_id: server,
                                        kind,
                                        ..Default::default()
                                    })),
                                    compact_player,
                                    cx,
                                );
                                cx.stop_propagation();
                            }
                        }),
                    )
            })
            .into_any_element()
    }
}
