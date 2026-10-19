use gpui::{prelude::*, *};
use gpui_component::{Icon, Sizable, StyledExt, animation::CssTransition};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "sidebar_inert_tests.rs"]
mod inert_tests;

struct FolderMotion {
    open: bool,
    progress: CssTransition,
}
impl FolderMotion {
    fn transition(value: f32) -> CssTransition {
        CssTransition::new(value, Duration::from_millis(200), [0.42, 0., 0.58, 1.])
    }
    fn new(open: bool) -> Self {
        Self {
            open,
            progress: Self::transition(f32::from(open)),
        }
    }
    fn frame(&mut self, open: bool, now: Instant) -> (f32, bool) {
        if self.open != open {
            // Motion restarts the full duration from the interrupted value.
            self.progress = Self::transition(self.progress.value(now).0);
            self.progress.retarget(f32::from(open), now);
            self.open = open;
        }
        self.progress.value(now)
    }
}

struct SectionMotion {
    open: bool,
    height: CssTransition,
    opacity: CssTransition,
}
impl SectionMotion {
    fn transition(value: f32) -> CssTransition {
        CssTransition::new(value, Duration::from_millis(200), [0.25, 0.1, 0.25, 1.])
    }
    fn new(open: bool) -> Self {
        Self {
            open,
            height: Self::transition(f32::from(open)),
            opacity: Self::transition(f32::from(open)),
        }
    }
    fn frame(&mut self, open: bool, now: Instant) -> (f32, f32, bool) {
        if self.open != open {
            if open && self.height.value(now).0 == 0. {
                // Opening from display:none has no previous opacity to transition.
                self.opacity = Self::transition(1.);
            } else {
                self.opacity.retarget(f32::from(open), now);
            }
            self.height.retarget(f32::from(open), now);
            self.open = open;
        }
        let (height, moving) = self.height.value(now);
        if !open && !moving {
            // The height transition's completion hides the original panel.
            self.opacity = Self::transition(0.);
        }
        let (opacity, fading) = self.opacity.value(now);
        (height, opacity, moving || fading)
    }
}

enum CollapseMotion {
    Folder(FolderMotion),
    Section(SectionMotion),
}
impl CollapseMotion {
    fn new(open: bool, section: bool) -> Self {
        if section {
            Self::Section(SectionMotion::new(open))
        } else {
            Self::Folder(FolderMotion::new(open))
        }
    }
    fn open(&self) -> bool {
        match self {
            Self::Folder(motion) => motion.open,
            Self::Section(motion) => motion.open,
        }
    }
    fn frame(&mut self, open: bool, now: Instant) -> (f32, f32, bool) {
        match self {
            Self::Folder(motion) => {
                let (value, animating) = motion.frame(open, now);
                (value, value, animating)
            }
            Self::Section(motion) => motion.frame(open, now),
        }
    }
}

struct CollapseState {
    motion: CollapseMotion,
    measured: Rc<Cell<Pixels>>,
    expanded_height: Pixels,
}

#[derive(IntoElement)]
pub(crate) struct FolderCollapse {
    pub id: SharedString,
    pub open: bool,
    pub content: AnyElement,
}
impl RenderOnce for FolderCollapse {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        collapse(self.id, self.open, self.content, false, window, cx)
    }
}

#[derive(IntoElement)]
pub(crate) struct SectionCollapse {
    pub id: SharedString,
    pub open: bool,
    pub content: AnyElement,
}
impl RenderOnce for SectionCollapse {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        collapse(self.id, self.open, self.content, true, window, cx)
    }
}

fn collapse(
    id: SharedString,
    open: bool,
    content: AnyElement,
    section: bool,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let state = window.use_keyed_state(id.clone(), cx, |_, _| CollapseState {
        motion: CollapseMotion::new(open, section),
        measured: Rc::new(Cell::new(px(0.))),
        expanded_height: px(0.),
    });
    let (progress, opacity, animating, height, measured) = state.update(cx, |state, cx| {
        if state.motion.open() != open {
            state.expanded_height = state.measured.get();
        }
        let (progress, opacity, animating) =
            state.motion.frame(open, cx.background_executor().now());
        (
            progress,
            opacity,
            animating,
            state.expanded_height,
            state.measured.clone(),
        )
    });
    if animating {
        window.request_animation_frame();
    }
    let selector = id;
    div()
        .debug_selector(move || selector.to_string())
        .v_flex()
        .w_full()
        .flex_shrink_0()
        .min_h_0()
        .overflow_hidden()
        .opacity(opacity)
        .inert(section && !open)
        .when(!open || animating, |div| div.h(height * progress))
        .when(progress == 0., |div| div.invisible())
        .child(div().v_flex().w_full().flex_shrink_0().child(content))
        .on_children_prepainted(move |bounds, _, _| {
            if let Some(bounds) = bounds.first() {
                measured.set(bounds.size.height);
            }
        })
}

#[derive(IntoElement)]
pub(crate) struct FolderChevron {
    pub id: SharedString,
    pub open: bool,
    pub size: Pixels,
}
impl RenderOnce for FolderChevron {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, _| FolderMotion::new(self.open));
        let (progress, animating) = state.update(cx, |state, cx| {
            state.frame(self.open, cx.background_executor().now())
        });
        if animating {
            window.request_animation_frame();
        }
        Icon::default()
            .path("native-icons/arrow-down.svg")
            .with_size(self.size)
            .rotate(Radians(std::f32::consts::PI * (1. + progress)))
    }
}

#[derive(IntoElement)]
pub(crate) struct SectionChevron {
    pub id: SharedString,
    pub open: bool,
    pub size: Pixels,
}
impl RenderOnce for SectionChevron {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, _| SectionMotion::new(self.open));
        let (progress, _, animating) = state.update(cx, |state, cx| {
            state.frame(self.open, cx.background_executor().now())
        });
        if animating {
            window.request_animation_frame();
        }
        Icon::default()
            .path("native-icons/arrow-down.svg")
            .with_size(self.size)
            .rotate(Radians(std::f32::consts::PI * (1. + progress)))
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    use gpui_component::Root;

    fn references() -> Vec<serde_json::Value> {
        #[cfg(target_os = "macos")]
        let source = include_str!("../assets/playlist-folder-motion-darwin-reference.json");
        #[cfg(target_os = "linux")]
        let source = include_str!("../assets/playlist-folder-motion-linux-reference.json");
        serde_json::from_str(source).unwrap()
    }

    #[test]
    fn folder_fades_and_chevrons_match_original_interrupted_motion() {
        for case in references() {
            let start = Instant::now();
            let mut open = case["initial"].as_bool().unwrap();
            let mut motion = FolderMotion::new(open);
            for frame in case["frames"].as_array().unwrap() {
                let milliseconds = frame["milliseconds"].as_u64().unwrap();
                for update in case["updates"].as_array().unwrap() {
                    if update[0].as_u64().unwrap() == milliseconds {
                        open = update[1].as_bool().unwrap();
                    }
                }
                let (value, _) = motion.frame(open, start + Duration::from_millis(milliseconds));
                assert!(
                    (value - frame["opacity"].as_f64().unwrap() as f32).abs() <= 0.001,
                    "opacity at {milliseconds}: {value} for {case}"
                );
                assert!(
                    (value * 180. - frame["rotation"].as_f64().unwrap() as f32).abs() <= 0.18,
                    "rotation at {milliseconds}: {value} for {case}"
                );
            }
        }
    }

    fn section_references() -> Vec<serde_json::Value> {
        #[cfg(target_os = "macos")]
        let source = include_str!("../assets/sidebar-section-motion-darwin-reference.json");
        #[cfg(target_os = "linux")]
        let source = include_str!("../assets/sidebar-section-motion-linux-reference.json");
        serde_json::from_str(source).unwrap()
    }

    #[test]
    fn section_fades_and_chevrons_match_original_css_reversals() {
        for case in section_references() {
            let start = Instant::now();
            let mut open = case["initial"].as_bool().unwrap();
            let mut motion = SectionMotion::new(open);
            for frame in case["frames"].as_array().unwrap() {
                let milliseconds = frame["milliseconds"].as_u64().unwrap();
                for update in case["updates"].as_array().unwrap() {
                    if update[0].as_u64().unwrap() == milliseconds {
                        open = update[1].as_bool().unwrap();
                    }
                }
                let (height, opacity, _) =
                    motion.frame(open, start + Duration::from_millis(milliseconds));
                assert!(
                    (opacity - frame["opacity"].as_f64().unwrap() as f32).abs() <= 0.001,
                    "opacity at {milliseconds}: {opacity} for {case}"
                );
                assert!(
                    (height * 180. - frame["rotation"].as_f64().unwrap() as f32).abs() <= 0.18,
                    "rotation at {milliseconds}: {height} for {case}"
                );
                assert_eq!(!open, frame["inert"].as_bool().unwrap());
            }
        }
    }

    struct Probe {
        section: bool,
        open: bool,
        height: f32,
        clicks: Rc<Cell<usize>>,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            div()
                .v_flex()
                .w(px(320.))
                .child(div().h(px(16.)).child(if self.section {
                    SectionChevron {
                        id: "test-chevron".into(),
                        open: self.open,
                        size: px(16.),
                    }
                    .into_any_element()
                } else {
                    FolderChevron {
                        id: "test-chevron".into(),
                        open: self.open,
                        size: px(16.),
                    }
                    .into_any_element()
                }))
                .child({
                    let content = div()
                        .id("test-child")
                        .role(Role::Button)
                        .aria_label("Panel contents")
                        .h(px(self.height))
                        .w_full()
                        .bg(rgb(0xff0000))
                        .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                        .into_any_element();
                    if self.section {
                        SectionCollapse {
                            id: "test-collapse".into(),
                            open: self.open,
                            content,
                        }
                        .into_any_element()
                    } else {
                        FolderCollapse {
                            id: "test-collapse".into(),
                            open: self.open,
                            content,
                        }
                        .into_any_element()
                    }
                })
                .child(div().debug_selector(|| "test-next".into()).h(px(16.)))
        }
    }

    #[gpui::test]
    fn folder_collapse_layout_and_clipped_clicks_match_original_frames(cx: &mut TestAppContext) {
        check_frames(cx, false);
    }

    #[gpui::test]
    fn section_collapse_layout_and_inert_contents_match_original_frames(cx: &mut TestAppContext) {
        check_frames(cx, true);
    }

    fn check_frames(cx: &mut TestAppContext, section: bool) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        for case in if section {
            section_references()
        } else {
            references()
        } {
            let height = if section {
                case["rem"].as_f64().unwrap() as f32 * 12.
            } else {
                case["height"].as_f64().unwrap() as f32
            };
            let clicks = Rc::new(Cell::new(0));
            let probe = cx.new(|_| Probe {
                section,
                open: case["initial"].as_bool().unwrap(),
                height,
                clicks: clicks.clone(),
            });
            let (_, window_cx) =
                cx.add_window_view(|window, cx| Root::new(probe.clone(), window, cx));
            window_cx.simulate_resize(size(px(800.), px(600.)));
            window_cx.update(|window, _| window.activate_accessibility_for_test());
            window_cx.run_until_parked();
            let mut previous = 0;
            for frame in case["frames"].as_array().unwrap() {
                let milliseconds = frame["milliseconds"].as_u64().unwrap();
                window_cx
                    .executor()
                    .advance_clock(Duration::from_millis(milliseconds - previous));
                previous = milliseconds;
                for update in case["updates"].as_array().unwrap() {
                    if update[0].as_u64().unwrap() == milliseconds {
                        probe.update(window_cx, |probe, cx| {
                            probe.open = update[1].as_bool().unwrap();
                            cx.notify();
                        });
                    }
                }
                window_cx.update(|window, _| window.refresh());
                window_cx.run_until_parked();
                let bounds = window_cx.debug_bounds("test-collapse").unwrap();
                // During an interrupted reopening, Motion briefly sets height:auto
                // to measure its target. Compare its interpolated value at that instant.
                let expected = frame[if frame["measuring"] == true {
                    "animatedHeight"
                } else {
                    "height"
                }]
                .as_f64()
                .unwrap() as f32;
                assert!(
                    (f32::from(bounds.size.height) - expected).abs() <= 1.,
                    "height at {milliseconds}: {bounds:?}, expected {expected}; {case}"
                );
                let next = window_cx.debug_bounds("test-next").unwrap();
                assert!((f32::from(next.top()) - expected - 16.).abs() <= 1.);
                let before = clicks.get();
                window_cx
                    .simulate_click(point(px(160.), px(16.25 + height * 0.5)), Modifiers::none());
                window_cx.run_until_parked();
                assert_eq!(
                    clicks.get() > before,
                    bounds.size.height >= px(height * 0.5 + 0.25)
                        && (!section || probe.read_with(window_cx, |probe, _| probe.open)),
                    "clipped click at {milliseconds}: {bounds:?}"
                );
                if section {
                    let visible = window_cx.update(|window, _| {
                        window
                            .accessibility_tree_for_test()
                            .unwrap()
                            .nodes
                            .iter()
                            .any(|(_, node)| node.label() == Some("Panel contents"))
                    });
                    assert_eq!(
                        visible,
                        frame["hidden"] == "false" && bounds.size.height > px(0.),
                        "accessibility at {milliseconds}: {frame}"
                    );
                }
            }
            window_cx.update(|window, _| window.remove_window());
        }
    }
}
