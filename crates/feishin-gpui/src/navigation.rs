use crate::app::{Feishin, Route};
use feishin_core::model::{ArtistTab, ItemKind};
use gpui::{Context, Pixels, Window, point, px};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Position {
    offset: Pixels,
    page: usize,
    loaded: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Visit {
    pub route: Route,
    position: Option<Position>,
    artist_tab: ArtistTab,
    artist_overview: bool,
}
impl Visit {
    pub(crate) fn reset_position(&mut self) {
        self.position = None;
    }
}
impl From<Route> for Visit {
    fn from(route: Route) -> Self {
        Self {
            route,
            position: None,
            artist_tab: ArtistTab::Albums,
            artist_overview: true,
        }
    }
}

#[derive(Debug)]
pub struct Restore {
    position: Position,
    frames: usize,
}

impl Feishin {
    pub(crate) fn uses_detail_scroll(&self) -> bool {
        self.scrolling_detail_header()
            || matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Album)
            || self.grouped_song_albums()
            || (self.list_config().display == "detail"
                && self.items.iter().all(|item| item.kind == ItemKind::Album))
    }

    fn navigation_offset(&self) -> Pixels {
        if self.artist_overview_active()
            || matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Song)
        {
            self.page_scroll.offset().y
        } else if self.uses_detail_scroll() {
            self.album_details_scroll.scroll_px_offset_for_scrollbar().y
        } else {
            self.library_scroll.0.borrow().base_handle.offset().y
        }
    }

    pub fn current_visit(&self) -> Visit {
        let position =
            if !self.is_browse_route() || matches!(self.route, Route::Home | Route::Queue) {
                None
            } else if let Some(restore) = &self.navigation_restore {
                Some(restore.position.clone())
            } else {
                Some(Position {
                    offset: self.navigation_offset(),
                    page: self.library_page,
                    loaded: self.items.len(),
                })
            };
        Visit {
            route: self.route.clone(),
            position,
            artist_tab: self.artist_tab,
            artist_overview: self.artist_overview,
        }
    }

    pub fn restore_visit(&mut self, visit: Visit, cx: &mut Context<Self>) {
        self.navigation_generation = self.navigation_generation.wrapping_add(1);
        self.route = visit.route;
        self.artist_tab = visit.artist_tab;
        self.artist_overview = visit.artist_overview;
        self.navigation_restore = visit.position.map(|position| Restore {
            position,
            frames: 0,
        });
        self.restore_view();
        self.reload_with_position(false, cx);
    }

    pub fn restored_library_page(&self) -> usize {
        self.navigation_restore
            .as_ref()
            .map_or(0, |restore| restore.position.page)
    }

    pub fn restored_local_limit(&self, limit: usize) -> usize {
        if self.list_config().pagination == "paginated" {
            limit
        } else {
            self.navigation_restore
                .as_ref()
                .map_or(limit, |restore| limit.max(restore.position.loaded))
        }
    }

    pub fn needs_restore_page(&self) -> bool {
        self.has_more
            && self.list_config().pagination != "paginated"
            && self
                .navigation_restore
                .as_ref()
                .is_some_and(|restore| self.items.len() < restore.position.loaded)
    }

    pub fn restore_navigation_scroll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.navigation_restore.is_none()
            || self.loading
            || (self.artist_overview_active()
                && self.artist_sections.values().any(|section| section.loading))
        {
            return;
        }
        let entity = cx.weak_entity();
        window.on_next_frame(move |_, cx| {
            entity
                .update(cx, |view, cx| {
                    let Some(restore) = &mut view.navigation_restore else {
                        return;
                    };
                    if view.loading {
                        return;
                    }
                    let target = restore.position.offset;
                    let finished = restore.frames >= 120;
                    restore.frames += 1;
                    // Wait for one laid-out frame after setting the position, so a clamped
                    // intermediate offset cannot replace the saved destination.
                    if finished
                        || (restore.frames > 1
                            && (view.navigation_offset() - target).abs() <= px(1.))
                    {
                        view.navigation_restore = None;
                        return;
                    }
                    if view.artist_overview_active()
                        || matches!(&view.route, Route::Detail(item) if item.kind == ItemKind::Song)
                    {
                        view.page_scroll.set_offset(point(px(0.), target));
                    } else if view.uses_detail_scroll() {
                        view.album_details_scroll
                            .set_offset_from_scrollbar(point(px(0.), target));
                    } else {
                        let mut scroll = view.library_scroll.0.borrow_mut();
                        scroll.deferred_scroll_to_item = None;
                        scroll.base_handle.set_offset(point(px(0.), target));
                    }
                    cx.notify();
                })
                .ok();
        });
    }
}
