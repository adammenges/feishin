use feishin_core::lists::{Grid, Size};
use gpui::{
    App, Context, Div, KeyDownEvent, Pixels, ScrollStrategy, Window, div, prelude::*, px, relative,
};
use gpui_component::{ActiveTheme, StyledExt};

#[derive(Clone, Copy)]
pub(crate) struct Layout {
    pub columns: usize,
    pub height: Pixels,
    pub width: Pixels,
    pub gap: Pixels,
    pub padding: Pixels,
}

fn columns(width: f32, grid: &Grid) -> usize {
    if grid.items_per_row_enabled && grid.items_per_row > 0 {
        return grid.items_per_row;
    }
    let columns = [
        (2560., 10),
        (1920., 8),
        (1440., 7),
        (1200., 6),
        (960., 5),
        (768., 4),
        (600., 3),
    ]
    .into_iter()
    .find_map(|(breakpoint, columns)| (width >= breakpoint).then_some(columns))
    .unwrap_or(2);
    if grid.size == Size::Large {
        (columns as f32 * 0.75).round().max(1.) as usize
    } else {
        columns
    }
}

impl Layout {
    pub fn new(outer_width: Pixels, grid: &Grid, font: Pixels, zoom: f32) -> Self {
        let padding = font * 0.75;
        let width = (outer_width - padding).max(px(1.));
        let columns = columns(f32::from(width) / zoom, grid);
        let metadata = if grid.size == Size::Compact {
            0.
        } else {
            grid.rows.iter().filter(|row| row.is_enabled).count() as f32 * 26. * zoom
        };
        Self {
            columns,
            height: width / columns as f32 + px(metadata),
            width,
            padding,
            gap: font
                * match grid.item_gap.as_str() {
                    "xs" => 0.25,
                    "sm" => 0.5,
                    "lg" => 1.,
                    "xl" => 1.5,
                    _ => 0.75,
                },
        }
    }
}

fn move_selection(
    order: &mut Vec<usize>,
    count: usize,
    columns: usize,
    key: &str,
    shift: bool,
) -> Option<usize> {
    if count == 0 || !matches!(key, "down" | "up" | "left" | "right") {
        return None;
    }
    let columns = columns.max(1);
    let current = order.last().copied().filter(|index| *index < count);
    let target = current.map_or(0, |index| match key {
        "down" if index / columns + 1 < count.div_ceil(columns) => (index + columns).min(count - 1),
        "up" if index >= columns => index - columns,
        "left" => index.saturating_sub(1),
        "right" => (index + 1).min(count - 1),
        _ => index,
    });
    if shift {
        let start = current.unwrap_or(target);
        for index in start.min(target)..=start.max(target) {
            if !order.contains(&index) {
                order.push(index);
            }
        }
        order.retain(|index| *index != target);
    } else {
        order.clear();
    }
    order.push(target);
    Some(target)
}

fn smart_scroll(count: usize, height: f32, viewport: f32, scroll: f32, index: usize) -> f32 {
    let last = (count as f32 * height - viewport).max(0.);
    let max = last.min(index as f32 * height);
    let min = (index as f32 * height - viewport + height).max(0.);
    if scroll >= min - viewport && scroll <= max + viewport {
        if scroll >= min && scroll <= max {
            scroll
        } else if scroll < min {
            min
        } else {
            max
        }
    } else {
        let middle = (min + (max - min) / 2.).round();
        if middle < (viewport / 2.).ceil() {
            0.
        } else if middle > last + (viewport / 2.).floor() {
            last
        } else {
            middle
        }
    }
}

impl crate::app::Feishin {
    pub(crate) fn library_grid_layout(&self, cx: &App) -> Layout {
        let width = if matches!(&self.route, crate::app::Route::Detail(item) if item.kind == feishin_core::model::ItemKind::Album)
        {
            let layout = self.album_content_layout(cx);
            self.library_width - layout.padding() - layout.songs_right_padding()
        } else {
            self.library_width
        };
        Layout::new(
            width,
            &self.list_config().grid,
            cx.theme().font_size,
            self.state.settings.zoom,
        )
    }

    pub(crate) fn library_grid_row(&self, indexes: &[usize], cx: &mut Context<Self>) -> Div {
        let layout = self.library_grid_layout(cx);
        div().w_full().h(layout.height).pr(layout.padding).child(
            div()
                .h_flex()
                .size_full()
                .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                .children(indexes.iter().map(|index| {
                    div()
                        .w(relative(1. / layout.columns as f32))
                        .h_full()
                        .flex_none()
                        .p(layout.gap)
                        .overflow_hidden()
                        .child(
                            self.item_card(
                                &self.items[*index],
                                *index,
                                crate::ui::CardSource::Library,
                                Some(
                                    (layout.width / layout.columns as f32 - layout.gap * 2.)
                                        .max(px(0.)),
                                ),
                                cx,
                            ),
                        )
                })),
        )
    }

    pub(crate) fn library_grid_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.list_config().display != "poster"
            || !self.library_focus.is_focused(window)
            || !matches!(
                event.keystroke.key.as_str(),
                "down" | "up" | "left" | "right"
            )
        {
            return;
        }
        window.prevent_default();
        cx.stop_propagation();
        let columns = self.library_grid_layout(cx).columns;
        self.grid_selection_order
            .retain(|index| self.selected.contains(index));
        let Some(index) = move_selection(
            &mut self.grid_selection_order,
            self.items.len(),
            columns,
            &event.keystroke.key,
            event.keystroke.modifiers.shift,
        ) else {
            return;
        };
        self.selected = self.grid_selection_order.iter().copied().collect();
        if self.scrolling_detail_header() {
            if let Some(row) = self.detail_row_position(index) {
                self.album_details_scroll.scroll_to_reveal_item(row);
            }
        } else {
            let groups = self.album_group_rows(columns);
            let count = groups
                .as_ref()
                .map_or(self.items.len().div_ceil(columns), Vec::len);
            let row = groups
                .as_ref()
                .and_then(|rows| rows.iter().position(|row| row.indexes.contains(&index)))
                .unwrap_or(index / columns);
            let mut scroll = self.library_scroll.0.borrow_mut();
            if let Some(size) = scroll.last_item_size {
                let height = if groups.is_some() {
                    size.contents.height / count.max(1) as f32
                } else {
                    self.library_grid_layout(cx).height
                };
                let offset = scroll.base_handle.offset();
                let y = smart_scroll(
                    count,
                    f32::from(height),
                    f32::from(size.item.height),
                    -f32::from(offset.y),
                    row,
                );
                scroll.deferred_scroll_to_item = None;
                scroll.base_handle.set_offset(gpui::point(offset.x, px(-y)));
            } else {
                drop(scroll);
                self.library_scroll
                    .scroll_to_item(row, ScrollStrategy::Center);
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{Layout, move_selection};
    use feishin_core::lists::{Grid, GridRow, Size};
    use gpui::px;
    #[test]
    fn sizing_and_keyboard_selection_match_original_callbacks() {
        let scrolls: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/grid-scroll-reference.json")).unwrap();
        for case in scrolls {
            let actual = super::smart_scroll(
                case["count"].as_u64().unwrap() as usize,
                case["rowHeight"].as_f64().unwrap() as f32,
                case["viewport"].as_f64().unwrap() as f32,
                case["scroll"].as_f64().unwrap() as f32,
                case["index"].as_u64().unwrap() as usize,
            );
            assert!(
                (actual - case["offset"].as_f64().unwrap() as f32).abs() <= 0.001,
                "{actual}: {case}"
            );
        }
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/library-grid-reference.json")).unwrap();
        for case in cases {
            let width = case["width"].as_f64().unwrap() as f32;
            let grid = Grid {
                size: match case["size"].as_str().unwrap() {
                    "compact" => Size::Compact,
                    "large" => Size::Large,
                    _ => Size::Default,
                },
                items_per_row_enabled: case["columns"] != 0,
                items_per_row: case["columns"].as_u64().unwrap() as usize,
                rows: vec![GridRow::default(); case["rows"].as_u64().unwrap() as usize],
                ..Default::default()
            };
            let layout = Layout::new(px(width + 12.), &grid, px(16.), 1.);
            let cols = layout.columns;
            assert_eq!(
                cols,
                case["columnCount"].as_u64().unwrap() as usize,
                "{case}"
            );
            let height = f32::from(layout.height);
            assert!(
                (height - case["itemHeight"].as_f64().unwrap() as f32).abs() < 0.001,
                "{case}"
            );
            assert_eq!(
                53usize.div_ceil(cols),
                case["rowCount"].as_u64().unwrap() as usize
            );
        }
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../assets/library-grid-navigation-reference.json"
        ))
        .unwrap();
        for case in cases {
            let mut order: Vec<usize> = serde_json::from_value(case["initial"].clone()).unwrap();
            let key = case["key"]
                .as_str()
                .unwrap()
                .trim_start_matches("Arrow")
                .to_lowercase();
            let target = if case["enabled"] == true {
                move_selection(
                    &mut order,
                    case["count"].as_u64().unwrap() as usize,
                    case["columns"].as_u64().unwrap() as usize,
                    &key,
                    case["shift"] == true,
                )
            } else {
                None
            };
            assert_eq!(
                serde_json::to_value(&order).unwrap(),
                case["selected"],
                "{case}"
            );
            assert_eq!(
                serde_json::to_value(target).unwrap(),
                case["scrolled"],
                "{case}"
            );
        }
    }
}

#[cfg(test)]
#[gpui::test]
fn cards_match_original_grid_padding_rows_gaps_and_rtl(cx: &mut gpui::TestAppContext) {
    use crate::app::{Feishin, Route};
    use feishin_core::{
        lists::{GridRow, Size},
        model::{Item, ItemKind},
        storage::State,
    };
    use gpui::*;
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};
    struct Probe {
        app: Entity<Feishin>,
        width: Pixels,
        detail: bool,
        _watch: Subscription,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let font = cx.theme().font_size;
            div()
                .debug_selector(|| "grid-probe".into())
                .w(self.width)
                .h(px(900.))
                .v_flex()
                .when(!self.detail, |probe| probe.py(font * 0.25))
                .children(self.app.update(cx, |view, cx| {
                    view.library_width = self.width;
                    let columns = view.library_grid_layout(cx).columns;
                    (0..view.items.len().div_ceil(columns).min(2))
                        .map(|row| {
                            let indexes = (row * columns
                                ..((row + 1) * columns).min(view.items.len()))
                                .collect::<Vec<_>>();
                            if self.detail {
                                view.detail_item_row(&indexes, columns, true, cx)
                            } else {
                                view.library_grid_row(&indexes, cx).into_any_element()
                            }
                        })
                        .collect::<Vec<_>>()
                }))
        }
    }
    cx.use_platform_text_system();
    cx.set_assets(crate::icons::Assets);
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::fonts::register_builtins(cx).unwrap();
    });
    let dir = std::env::temp_dir().join(format!("feishin-grid-layout-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        let mut state = State::default();
        state.settings.home.feature = false;
        state.settings.home.items.clear();
        state.settings.sidebar.playlist_list = false;
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        let probe = cx.new(|cx| Probe {
            app: app.clone(),
            width: px(300.),
            detail: false,
            _watch: cx.observe(&app, |_, _, cx| cx.notify()),
        });
        captured.replace(Some((app, probe.clone())));
        Root::new(probe, window, cx)
    });
    let (app, probe) = captured.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1800.), px(1200.)));
    cx.run_until_parked();
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../assets/library-grid-layout-reference.json")).unwrap();
    for (case, detail) in cases
        .into_iter()
        .flat_map(|case| [(case.clone(), false), (case, true)])
    {
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Library(ItemKind::Album);
                view.state.settings.language = if case["rtl"] == true { "ar" } else { "en" }.into();
                cx.set_global(crate::appearance::Language(
                    view.state.settings.language.clone(),
                ));
                view.apply_theme(window, cx);
                let font = px(case["font"].as_f64().unwrap() as f32);
                gpui_component::Theme::global_mut(cx).font_size = font;
                window.set_rem_size(font);
                view.items = (0..case["count"].as_u64().unwrap())
                    .map(|index| Item {
                        id: index.to_string(),
                        name: "Title".into(),
                        kind: ItemKind::Album,
                        ..Default::default()
                    })
                    .collect();
                let config = view
                    .state
                    .settings
                    .lists
                    .entry("album".into())
                    .or_insert_with(|| feishin_core::lists::default_for("album"));
                config.grid.size = match case["size"].as_str().unwrap() {
                    "compact" => Size::Compact,
                    "large" => Size::Large,
                    _ => Size::Default,
                };
                config.grid.item_gap = case["gap"].as_str().unwrap().into();
                config.grid.rows = ["name", "year", "songCount"]
                    .into_iter()
                    .map(|id| GridRow {
                        id: id.into(),
                        ..Default::default()
                    })
                    .collect();
                cx.notify();
            });
            probe.update(cx, |view, cx| {
                view.width = px(case["width"].as_f64().unwrap() as f32);
                view.detail = detail;
                cx.notify();
            });
        });
        cx.run_until_parked();
        if case["hover"] == true {
            crate::tests::show_card_controls(cx, 0);
        }
        let origin = cx.debug_bounds("grid-probe").unwrap().origin;
        for (part, selector) in [("cards", "item-card"), ("art", "card-open")] {
            for (index, expected) in case[part].as_array().unwrap().iter().enumerate() {
                let actual = cx.debug_bounds(&format!("{selector}-{index}")).unwrap();
                for (actual, expected) in [
                    actual.origin.x - origin.x,
                    actual.origin.y - origin.y,
                    actual.size.width,
                    actual.size.height,
                ]
                .into_iter()
                .zip(expected.as_array().unwrap())
                {
                    assert!(
                        (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                        "{selector}-{index}: {actual:?} vs {expected}: {case}"
                    );
                }
            }
        }
        if let Some(controls) = case["controls"].as_object() {
            for (name, selector) in [
                ("primary", "library-play-0"),
                ("left", "library-next-0"),
                ("right", "library-last-0"),
            ] {
                let actual = cx.debug_bounds(selector).unwrap();
                for (actual, expected) in [
                    actual.origin.x - origin.x,
                    actual.origin.y - origin.y,
                    actual.size.width,
                    actual.size.height,
                ]
                .into_iter()
                .zip(controls[name].as_array().unwrap())
                {
                    assert!(
                        (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                        "{selector}: {actual:?} vs {expected}: {case}"
                    );
                }
            }
        }
    }
    drop(app);
    std::fs::remove_dir_all(dir).ok();
}
