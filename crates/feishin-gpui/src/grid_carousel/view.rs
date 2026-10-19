use super::{Drag, Page, Source, State, columns};
use crate::{app::Feishin, ui::CardSource};
use feishin_core::model::{Item, ItemKind, Sort};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, Sizable, StyledExt, button::ButtonVariants};
use std::cell::RefCell;

struct Content<'a> {
    state: &'a RefCell<State>,
    items: Vec<(usize, &'a Item)>,
    kind: ItemKind,
    title: String,
    loading: bool,
    loading_more: bool,
    has_more: bool,
    error: &'a str,
    refresh: bool,
    hidden: bool,
}

fn grid(columns: usize, gap: Pixels) -> Div {
    div()
        .grid()
        .grid_cols(columns as u16)
        .gap(gap)
        .w_full()
        .min_w_0()
}

fn cell(index: usize, columns: usize, rtl: bool) -> Div {
    div()
        .min_w_0()
        .min_h_0()
        .col_start(if rtl {
            columns - index % columns
        } else {
            index % columns + 1
        } as i16)
        .row_start((index / columns + 1) as i16)
}

impl Feishin {
    fn carousel_content(&self, source: Source) -> Option<Content<'_>> {
        let (section, kind, title, refresh) = match source {
            Source::Home(kind) => {
                let section = self.home_sections.get(&kind)?;
                let server = self
                    .api()
                    .map_or(feishin_core::model::ServerType::Navidrome, |api| {
                        api.server.kind
                    });
                (
                    section,
                    kind.query(server, 0).kind,
                    self.tr(kind.title_key()),
                    true,
                )
            }
            Source::Album(index) => {
                let section = self.album_related.get(index)?;
                (
                    &section.state,
                    ItemKind::Album,
                    section.title.clone(),
                    section.query.sort == Sort::Random,
                )
            }
            Source::RelatedArtists => {
                let section = self.artist_sections.get("similarArtists")?;
                let items = self
                    .related_artist_indices()
                    .into_iter()
                    .map(|index| (index, &section.items[index]))
                    .collect::<Vec<_>>();
                return Some(Content {
                    state: &section.grid,
                    hidden: items.is_empty() && !section.loading && section.error.is_empty(),
                    items,
                    kind: ItemKind::AlbumArtist,
                    title: self.tr("page.albumArtistDetail.relatedArtists"),
                    loading: section.loading,
                    loading_more: false,
                    has_more: false,
                    error: &section.error,
                    refresh: false,
                });
            }
        };
        Some(Content {
            state: &section.grid,
            items: section.items.iter().enumerate().collect(),
            kind,
            title,
            loading: section.loading,
            loading_more: section.loading_more,
            has_more: section.has_more,
            error: &section.error,
            refresh,
            hidden: section.hidden(),
        })
    }

    fn carousel_selector(&self, source: Source, part: &str) -> String {
        match source {
            Source::RelatedArtists => format!("artist-similar-{part}"),
            Source::Album(index) => format!("album-related-{part}-{index}"),
            Source::Home(kind) => {
                let index = self
                    .state
                    .settings
                    .home
                    .enabled()
                    .position(|item| item == kind)
                    .unwrap_or(0);
                if part == "section" {
                    format!("home-section-{kind:?}")
                } else {
                    format!("home-{part}-{index}")
                }
            }
        }
    }

    fn carousel_load(&mut self, source: Source, more: bool, cx: &mut Context<Self>) {
        match source {
            Source::Home(kind) => self.load_home_section(kind, more, cx),
            Source::Album(index) => self.load_album_related_page(index, more, cx),
            Source::RelatedArtists if !more => self.load_artist_section("similarArtists", cx),
            Source::RelatedArtists => {}
        }
    }

    pub(crate) fn carousel_step(&mut self, source: Source, next: bool, cx: &mut Context<Self>) {
        if let Some(content) = self.carousel_content(source) {
            let mut state = content.state.borrow_mut();
            let columns = columns(state.width / self.state.settings.zoom);
            let count = if content.has_more {
                usize::MAX
            } else {
                content.items.len()
            };
            state
                .carousel
                .step(next, count, columns, cx.background_executor().now());
            cx.notify();
        }
    }

    fn carousel_wheel(&mut self, source: Source, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        if let Some(content) = self.carousel_content(source) {
            let mut state = content.state.borrow_mut();
            let columns = columns(state.width / self.state.settings.zoom);
            let count = if content.has_more {
                usize::MAX
            } else {
                content.items.len()
            };
            if state.carousel.wheel(
                -f32::from(event.delta.pixel_delta(px(1.)).y),
                event.modifiers.shift,
                count,
                columns,
                cx.background_executor().now(),
            ) {
                cx.notify();
            }
        }
    }

    fn carousel_drag_move(
        &mut self,
        source: Source,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        if let Some(content) = self.carousel_content(source)
            && let Some(drag) = content.state.borrow_mut().drag.as_mut()
        {
            drag.update(
                f32::from(event.position.x) / self.state.settings.zoom,
                cx.background_executor().now(),
            );
        }
    }

    fn carousel_drag_end(&mut self, source: Source, event: &MouseUpEvent, cx: &mut Context<Self>) {
        if let Some(content) = self.carousel_content(source) {
            let mut state = content.state.borrow_mut();
            if let Some(drag) = state.drag.take() {
                let (offset, velocity) =
                    drag.finish(f32::from(event.position.x) / self.state.settings.zoom);
                let columns = columns(state.width / self.state.settings.zoom);
                let count = if content.has_more {
                    usize::MAX
                } else {
                    content.items.len()
                };
                state.carousel.swipe(
                    offset,
                    velocity,
                    false,
                    count,
                    columns,
                    cx.background_executor().now(),
                );
                cx.notify();
            }
        }
    }

    pub(crate) fn grid_carousel_view(
        &self,
        source: Source,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let content = self.carousel_content(source)?;
        if content.hidden {
            return None;
        }
        let font = cx.theme().font_size;
        let zoom = self.state.settings.zoom;
        let rtl = crate::appearance::rtl(cx);
        let initial = content.loading && content.items.is_empty();
        let (width, columns, frame, page) = {
            let mut state = content.state.borrow_mut();
            let width = state.width;
            let columns = columns(width / zoom);
            let page = if initial {
                Page {
                    items: 0..0,
                    placeholders: 0..columns,
                    disabled: [true, true],
                    request: false,
                }
            } else {
                state.sync(
                    content.items.len(),
                    columns,
                    content.has_more,
                    content.loading_more,
                )
            };
            (
                width,
                columns,
                state.carousel.frame(cx.background_executor().now()),
                page,
            )
        };
        if frame.animating {
            window.request_animation_frame();
        }
        if page.request && content.has_more && !content.loading && content.error.is_empty() {
            cx.defer_in(window, move |view, _, cx| {
                view.carousel_load(source, true, cx)
            });
        }
        let swipe_id = self.carousel_selector(source, "swipe");
        let swipe_selector = swipe_id.clone();
        let mut title = div()
            .id(SharedString::from(swipe_id))
            .debug_selector(move || swipe_selector.clone())
            .h_flex()
            .min_w_0()
            .flex_1()
            .gap(font * 0.25)
            .items_center()
            .cursor(CursorStyle::OpenHand)
            .when(rtl, |row| row.flex_row_reverse())
            .child(
                div()
                    .text_size(font * 1.5)
                    .line_height(font * 2.)
                    .font_weight(FontWeight::BOLD)
                    .child(content.title),
            )
            .when(source == Source::RelatedArtists, |row| {
                row.child(
                    div()
                        .flex_1()
                        .ml(font * 0.5)
                        .h(px(1.))
                        .bg(cx.theme().border),
                )
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, _, cx| {
                    if let Some(content) = view.carousel_content(source) {
                        content.state.borrow_mut().drag = Some(Drag::new(
                            f32::from(event.position.x) / view.state.settings.zoom,
                            cx.background_executor().now(),
                        ));
                    }
                }),
            )
            .on_mouse_move(
                cx.listener(move |view, event, _, cx| view.carousel_drag_move(source, event, cx)),
            )
            .on_mouse_move_out(
                cx.listener(move |view, event, _, cx| view.carousel_drag_move(source, event, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |view, event, _, cx| view.carousel_drag_end(source, event, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |view, event, _, cx| view.carousel_drag_end(source, event, cx)),
            );
        if content.refresh && !initial {
            let id = self.carousel_selector(source, "refresh");
            let selector = id.clone();
            title = title.child(
                crate::styles::button(SharedString::from(id), cx)
                    .debug_selector(move || selector.clone())
                    .ghost()
                    .small()
                    .label("↻")
                    .tooltip(self.tr("Refresh"))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.carousel_load(source, false, cx)),
                    ),
            );
        }
        let mut heading = div()
            .h_flex()
            .w_full()
            .gap(font * 0.25)
            .items_center()
            .when(rtl, |row| row.flex_row_reverse())
            .child(title);
        for (next, label, glyph) in [(false, "Previous page", "‹"), (true, "Next page", "›")] {
            let id = self.carousel_selector(source, if next { "next" } else { "prev" });
            let selector = id.clone();
            heading = heading.child(
                crate::styles::button(SharedString::from(id), cx)
                    .debug_selector(move || selector.clone())
                    .ghost()
                    .small()
                    .label(glyph)
                    .tooltip(self.tr(label))
                    .disabled(page.disabled[usize::from(next)])
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.carousel_step(source, next, cx)),
                    ),
            );
        }
        let page = if initial {
            page
        } else {
            Page::new(
                frame.page,
                content.items.len(),
                columns,
                content.has_more,
                content.loading_more,
            )
        };
        let card_width =
            px(((width - f32::from(font * 0.75) * (columns - 1) as f32) / columns as f32).max(1.));
        let grid_id = self.carousel_selector(source, "grid");
        let grid_selector = grid_id.clone();
        let mut cards = grid(columns, font * 0.75)
            .id(SharedString::from(grid_id))
            .debug_selector(move || grid_selector.clone())
            .relative()
            .left(px(frame.x * zoom))
            .opacity(frame.opacity)
            .overflow_hidden()
            .on_scroll_wheel(
                cx.listener(move |view, event, _, cx| view.carousel_wheel(source, event, cx)),
            );
        for (slot, index) in page.items.enumerate() {
            let (item_index, item) = content.items[index];
            let selector = format!("{}-{item_index}", self.carousel_selector(source, "card"));
            cards = cards.child(
                cell(slot, columns, rtl)
                    .id(SharedString::from(selector.clone()))
                    .debug_selector(move || selector.clone())
                    .child(self.item_card(
                        item,
                        item_index,
                        CardSource::Carousel,
                        Some(card_width),
                        cx,
                    )),
            );
        }
        let key = match content.kind {
            ItemKind::Song => "song",
            ItemKind::Playlist => "playlist",
            ItemKind::AlbumArtist | ItemKind::Artist => "albumArtist",
            _ => "album",
        };
        let config = self
            .state
            .settings
            .lists
            .get(key)
            .cloned()
            .unwrap_or_else(|| feishin_core::lists::default_for(key));
        let placeholder_count = page.placeholders.len();
        for (slot, index) in page.placeholders.enumerate() {
            let selector = format!("{}-{index}", self.carousel_selector(source, "placeholder"));
            cards = cards.child(
                cell(columns - placeholder_count + slot, columns, rtl)
                    .id(SharedString::from(selector.clone()))
                    .debug_selector(move || selector.clone())
                    .v_flex()
                    .gap(font * 0.25)
                    .child(
                        div()
                            .w_full()
                            .h(card_width)
                            .bg(cx.theme().secondary)
                            .rounded_md()
                            .when(content.kind == ItemKind::AlbumArtist, |art| {
                                art.rounded_full()
                            }),
                    )
                    .children(
                        config
                            .grid
                            .rows
                            .iter()
                            .filter(|row| row.is_enabled)
                            .map(|_| div().w_full().h(font * 1.5)),
                    ),
            );
        }
        let section_id = if source == Source::RelatedArtists {
            "artist-overview-similarArtists".to_owned()
        } else {
            self.carousel_selector(source, "section")
        };
        let selector = section_id.clone();
        let measure = cx.entity().downgrade();
        let mut result = div()
            .id(SharedString::from(section_id))
            .debug_selector(move || selector.clone())
            .v_flex()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .gap(font * 0.75)
            .child(heading)
            .child(
                div()
                    .w_full()
                    .relative()
                    .overflow_hidden()
                    .child(cards)
                    .child(
                        canvas(
                            move |bounds, window, cx| {
                                let measure = measure.clone();
                                window.defer(cx, move |_, cx| {
                                    measure
                                        .update(cx, |view, cx| {
                                            if let Some(content) = view.carousel_content(source) {
                                                let mut state = content.state.borrow_mut();
                                                if (state.width - f32::from(bounds.size.width))
                                                    .abs()
                                                    > 0.01
                                                {
                                                    state.width = f32::from(bounds.size.width);
                                                    cx.notify();
                                                }
                                            }
                                        })
                                        .ok();
                                });
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    ),
            );
        if !content.error.is_empty() {
            result = result.child(content.error.to_owned()).child(
                crate::styles::button(
                    SharedString::from(self.carousel_selector(source, "retry")),
                    cx,
                )
                .label(self.tr("Retry"))
                .on_click(cx.listener(move |view, _, _, cx| view.carousel_load(source, false, cx))),
            );
        }
        Some(result.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*, px};
    #[gpui::test]
    fn carousel_grid_matches_original_css_at_breakpoints_zoom_and_rtl(cx: &mut TestAppContext) {
        struct Probe(serde_json::Value, f32);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let width = self.0["width"].as_f64().unwrap() as f32;
                let columns = super::columns(width);
                let rtl = self.0["rtl"] == true;
                div().w(px(width * self.1)).child(
                    super::grid(
                        columns,
                        px(self.0["font"].as_f64().unwrap() as f32 * 0.75 * self.1),
                    )
                    .debug_selector(|| "carousel-grid".into())
                    .children((0..17).map(|index| {
                        super::cell(index, columns, rtl)
                            .debug_selector(move || format!("carousel-card-{index}"))
                            .h(px(89. * self.1))
                    })),
                )
            }
        }
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../assets/grid-carousel-layout-reference.json"
        ))
        .unwrap();
        let (probe, cx) = cx.add_window_view(|_, _| Probe(cases[0].clone(), 1.));
        cx.simulate_resize(gpui::size(px(3000.), px(2000.)));
        for zoom in [1., 1.5] {
            for case in &cases {
                cx.update(|_, cx| {
                    probe.update(cx, |view, cx| {
                        *view = Probe(case.clone(), zoom);
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                let origin = cx.debug_bounds("carousel-grid").unwrap().origin;
                for index in 0..17 {
                    let actual = cx.debug_bounds(&format!("carousel-card-{index}")).unwrap();
                    for (axis, (actual, expected)) in [
                        actual.origin.x - origin.x,
                        actual.origin.y - origin.y,
                        actual.size.width,
                        actual.size.height,
                    ]
                    .into_iter()
                    .zip(case["cards"][index].as_array().unwrap())
                    .enumerate()
                    {
                        let offset = if axis < 2 {
                            case["grid"][axis].as_f64().unwrap()
                        } else {
                            0.
                        };
                        let expected = (expected.as_f64().unwrap() - offset) as f32 * zoom;
                        assert!(
                            (f32::from(actual) - expected).abs() < 1.01,
                            "{case} zoom {zoom} card {index} axis {axis}: {actual:?} vs {expected}"
                        );
                    }
                }
            }
        }
    }
}
