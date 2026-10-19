use gpui_component::animation::CssTransition;
use std::time::{Duration, Instant};
mod view;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Home(feishin_core::home::Kind),
    Album(usize),
    RelatedArtists,
}

pub fn columns(width: f32) -> usize {
    [520., 720., 960., 1152., 1280., 1440.]
        .into_iter()
        .filter(|breakpoint| width >= *breakpoint)
        .count()
        + 2
}

#[derive(Default)]
pub struct State {
    pub carousel: Carousel,
    pub width: f32,
    pub drag: Option<Drag>,
    needs_more: Option<bool>,
}

pub struct Page {
    pub items: std::ops::Range<usize>,
    pub placeholders: std::ops::Range<usize>,
    pub disabled: [bool; 2],
    pub request: bool,
}
impl Page {
    fn new(page: usize, count: usize, columns: usize, has_more: bool, loading: bool) -> Self {
        let start = page.saturating_mul(columns);
        let end = start.saturating_add(columns);
        let items = start.min(count)..end.min(count);
        let missing_start = start.saturating_add(items.len());
        Self {
            items,
            placeholders: missing_start..if has_more && loading {
                end
            } else {
                missing_start
            },
            disabled: [page == 0, end >= count && !has_more],
            request: false,
        }
    }
}
impl State {
    pub fn sync(&mut self, count: usize, columns: usize, has_more: bool, loading: bool) -> Page {
        let mut page = Page::new(self.carousel.page, count, columns, has_more, loading);
        let needed = page.items.len() + page.placeholders.len() < columns;
        page.request = needed && self.needs_more != Some(needed);
        self.needs_more = Some(needed);
        page
    }
}

pub struct Drag {
    history: Vec<(f32, Instant)>,
}
impl Drag {
    pub fn new(x: f32, now: Instant) -> Self {
        Self {
            history: vec![(x, now)],
        }
    }
    pub fn update(&mut self, x: f32, now: Instant) {
        self.history.push((x, now));
    }
    pub fn finish(self, x: f32) -> (f32, f32) {
        let last = self.history.last().unwrap();
        let mut first = self
            .history
            .iter()
            .rposition(|(_, at)| last.1.saturating_duration_since(*at) > Duration::from_millis(100))
            .unwrap_or(0);
        if first == 0
            && self.history.len() > 2
            && last.1.saturating_duration_since(self.history[0].1) > Duration::from_millis(200)
        {
            first = 1;
        }
        let elapsed = last
            .1
            .saturating_duration_since(self.history[first].1)
            .as_secs_f32();
        let velocity = if elapsed > 0. {
            (last.0 - self.history[first].0) / elapsed
        } else {
            0.
        };
        (x - self.history[0].0, velocity)
    }
}

#[derive(Default)]
pub struct Carousel {
    pub page: usize,
    shown: usize,
    next: bool,
    wheel_at: Option<Instant>,
    swipe_at: Option<Instant>,
    transition: Option<Transition>,
}
struct Transition {
    exiting: bool,
    opacity: CssTransition,
    x: CssTransition,
}
#[derive(Debug)]
pub struct Frame {
    pub page: usize,
    pub opacity: f32,
    pub x: f32,
    pub animating: bool,
}
impl Transition {
    fn new(exiting: bool, from: (f32, f32), to: (f32, f32), now: Instant) -> Self {
        let curve = if exiting {
            [0.42, 0., 1., 1.]
        } else {
            [0., 0., 0.58, 1.]
        };
        let mut opacity = CssTransition::new(from.0, Duration::from_millis(300), curve);
        let mut x = CssTransition::new(from.1, Duration::from_millis(300), curve);
        opacity.retarget(to.0, now);
        x.retarget(to.1, now);
        Self {
            exiting,
            opacity,
            x,
        }
    }
}
impl Carousel {
    pub fn disabled(&self, count: usize, columns: usize) -> [bool; 2] {
        [
            self.page == 0,
            self.page.saturating_add(1).saturating_mul(columns) >= count,
        ]
    }
    pub fn step(&mut self, next: bool, count: usize, columns: usize, now: Instant) -> bool {
        if self.disabled(count, columns)[usize::from(next)] {
            return false;
        }
        self.go(if next { self.page + 1 } else { self.page - 1 }, next, now);
        true
    }
    fn go(&mut self, page: usize, next: bool, now: Instant) {
        let current = self.frame(now);
        self.page = page;
        self.next = next;
        if self.shown == page {
            self.transition = Some(Transition::new(
                false,
                (current.opacity, current.x),
                (1., 0.),
                now,
            ));
        } else if self.transition.as_ref().is_none_or(|phase| !phase.exiting) {
            self.transition = Some(Transition::new(
                true,
                (current.opacity, current.x),
                (0., if next { -100. } else { 100. }),
                now,
            ));
        }
    }
    pub fn wheel(
        &mut self,
        delta: f32,
        shift: bool,
        count: usize,
        columns: usize,
        now: Instant,
    ) -> bool {
        if !shift
            || !delta.is_finite()
            || delta.abs() < 10.
            || self
                .wheel_at
                .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(250))
        {
            return false;
        }
        let changed = self.step(delta > 0., count, columns, now);
        if changed {
            self.wheel_at = Some(now);
        }
        changed
    }
    pub fn swipe(
        &mut self,
        offset: f32,
        velocity: f32,
        interactive: bool,
        count: usize,
        columns: usize,
        now: Instant,
    ) -> bool {
        if interactive
            || self
                .swipe_at
                .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(300))
        {
            return false;
        }
        if offset.abs() > 50. || velocity.abs() > 500. {
            self.swipe_at = Some(now);
            if offset != 0. {
                return self.step(offset < 0., count, columns, now);
            }
        }
        false
    }
    pub fn frame(&mut self, now: Instant) -> Frame {
        if let Some(phase) = &self.transition {
            let (opacity, animating) = phase.opacity.value(now);
            let (x, moving) = phase.x.value(now);
            if animating || moving {
                return Frame {
                    page: self.shown,
                    opacity,
                    x,
                    animating: true,
                };
            }
            if phase.exiting {
                self.shown = self.page;
                self.transition = Some(Transition::new(
                    false,
                    (0., if self.next { 100. } else { -100. }),
                    (1., 0.),
                    now,
                ));
                return self.frame(now);
            }
            self.transition = None;
        }
        Frame {
            page: self.shown,
            opacity: 1.,
            x: 0.,
            animating: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Carousel;
    use std::time::{Duration, Instant};
    #[test]
    fn infinite_loading_placeholders_resize_and_refetch_match_original() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../assets/infinite-grid-carousel-reference.json"
        ))
        .unwrap();
        for case in cases {
            let mut state = super::State::default();
            let mut width = case["width"].as_f64().unwrap() as f32;
            let mut ids = (0..case["count"].as_u64().unwrap()).collect::<Vec<_>>();
            let mut loading = false;
            let mut has_more = true;
            let mut requests = 0;
            for frame in case["frames"].as_array().unwrap() {
                let action = &frame["action"];
                match action["kind"].as_str().unwrap() {
                    "next" | "prev" => {
                        state.carousel.step(
                            action["kind"] == "next",
                            if has_more { usize::MAX } else { ids.len() },
                            super::columns(width),
                            Instant::now(),
                        );
                    }
                    "resize" => width = action["width"].as_f64().unwrap() as f32,
                    "loading" => loading = action["value"].as_bool().unwrap(),
                    "append" => {
                        let start = ids.len() as u64;
                        ids.extend(start..start + action["count"].as_u64().unwrap());
                        loading = false;
                        has_more = action["hasMore"].as_bool().unwrap();
                    }
                    "replace" => {
                        ids = serde_json::from_value(action["ids"].clone()).unwrap();
                        loading = false;
                        has_more = action["hasMore"].as_bool().unwrap();
                    }
                    "initial" => {}
                    _ => unreachable!(),
                }
                let page = state.sync(ids.len(), super::columns(width), has_more, loading);
                requests += usize::from(page.request);
                let visible = page
                    .items
                    .map(|index| serde_json::json!(ids[index]))
                    .chain(
                        page.placeholders
                            .map(|index| serde_json::json!(format!("placeholder-{index}"))),
                    )
                    .collect::<Vec<_>>();
                assert_eq!(
                    serde_json::json!({ "page":state.carousel.page, "width":width as u32, "count":ids.len(), "loading":loading, "hasMore":has_more, "visible":visible, "disabled":page.disabled, "fetchCalls":requests }),
                    frame["result"],
                    "{case}: {action}"
                );
            }
        }
    }
    #[test]
    fn navigation_wheel_and_swipes_match_original_grid_carousel() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/grid-carousel-reference.json")).unwrap();
        let start = Instant::now();
        for case in cases {
            let width = case["width"].as_f64().unwrap() as f32;
            let columns = [520., 720., 960., 1152., 1280., 1440.]
                .into_iter()
                .filter(|point| width >= *point)
                .count()
                + 2;
            let count = case["count"].as_u64().unwrap() as usize;
            let mut carousel = Carousel::default();
            for action in case["actions"].as_array().unwrap() {
                let now = start + Duration::from_millis(action["ms"].as_u64().unwrap());
                match action["kind"].as_str().unwrap() {
                    "next" | "prev" => {
                        carousel.step(action["kind"] == "next", count, columns, now);
                    }
                    "wheel" => {
                        carousel.wheel(
                            action["delta"].as_f64().unwrap() as f32,
                            action["shift"].as_bool().unwrap(),
                            count,
                            columns,
                            now,
                        );
                    }
                    _ => {
                        carousel.swipe(
                            action["offset"].as_f64().unwrap() as f32,
                            action["velocity"].as_f64().unwrap() as f32,
                            action["interactive"].as_bool().unwrap(),
                            count,
                            columns,
                            now,
                        );
                    }
                }
                assert_eq!(
                    carousel.page,
                    action["result"]["page"].as_u64().unwrap() as usize,
                    "{case}: {action}"
                );
                assert_eq!(
                    serde_json::json!(carousel.disabled(count, columns)),
                    action["result"]["disabled"],
                    "{action}"
                );
                let visible = (carousel.page * columns..((carousel.page + 1) * columns).min(count))
                    .collect::<Vec<_>>();
                assert_eq!(
                    serde_json::json!(visible),
                    action["result"]["visible"],
                    "{action}"
                );
            }
        }
    }
    #[test]
    fn page_exit_wait_entry_and_rapid_navigation_match_original_motion() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../assets/grid-carousel-motion-reference.json"
        ))
        .unwrap();
        let start = Instant::now();
        for case in cases {
            let mut carousel = Carousel::default();
            for frame in case["frames"].as_array().unwrap() {
                let now = start + Duration::from_millis(frame["time"].as_u64().unwrap());
                match frame["action"].as_str() {
                    Some("next") => carousel.go(carousel.page + 1, true, now),
                    Some("prev") => carousel.go(0, false, now),
                    _ => {}
                }
                let actual = carousel.frame(now);
                let expected = &frame["rows"][0];
                assert_eq!(
                    actual.page,
                    expected["page"].as_u64().unwrap() as usize,
                    "{frame}"
                );
                assert!(
                    (actual.opacity - expected["opacity"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{frame}: {actual:?}"
                );
                assert!(
                    (actual.x - expected["x"].as_f64().unwrap() as f32).abs() < 0.05,
                    "{frame}: {actual:?}"
                );
            }
        }
    }
}
