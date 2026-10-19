use gpui::{prelude::*, *};
use gpui_component::animation::CssTransition;
use std::time::{Duration, Instant};

struct ArtworkMotion {
    shown: bool,
    motion: CssTransition,
    initial_fade: Option<CssTransition>,
}
impl ArtworkMotion {
    fn transition(value: f32, milliseconds: u64) -> CssTransition {
        CssTransition::new(
            value,
            Duration::from_millis(milliseconds),
            [0.42, 0., 0.58, 1.],
        )
    }
    fn new(shown: bool, now: Instant) -> Self {
        Self {
            shown,
            motion: Self::transition(f32::from(shown), 300),
            initial_fade: shown.then(|| {
                let mut fade = Self::transition(0., 200);
                fade.retarget(1., now);
                fade
            }),
        }
    }
    fn frame(&mut self, shown: bool, now: Instant) -> Option<(f32, f32, bool)> {
        if self.shown != shown {
            self.initial_fade = None;
            self.motion = Self::transition(self.motion.value(now).0, 300);
            self.motion.retarget(f32::from(shown), now);
            self.shown = shown;
        }
        let (position, moving) = self.motion.value(now);
        if !shown && !moving {
            return None;
        }
        let (opacity, fading) = self
            .initial_fade
            .as_ref()
            .map_or((position, moving), |fade| fade.value(now));
        Some((position, opacity, moving || fading))
    }
}

#[derive(IntoElement)]
pub(crate) struct ArtworkPresence {
    pub shown: bool,
    pub width: Pixels,
    pub distance: Pixels,
    pub content: AnyElement,
}
impl RenderOnce for ArtworkPresence {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state("sidebar-artwork-presence", cx, |_, cx| {
            ArtworkMotion::new(self.shown, cx.background_executor().now())
        });
        let frame = state.update(cx, |state, cx| {
            state.frame(self.shown, cx.background_executor().now())
        });
        let Some((position, opacity, animating)) = frame else {
            return div().hidden();
        };
        if animating {
            window.request_animation_frame();
        }
        // The original SidebarImage does not forward AnimatePresence's layout ref,
        // so its exiting image keeps its space until the exit completes.
        div()
            .relative()
            .top(self.distance * (1. - position))
            .size(self.width)
            .flex_shrink_0()
            .opacity(opacity)
            .child(self.content)
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    use gpui_component::{Root, StyledExt};
    use std::{cell::Cell, rc::Rc};

    struct Probe {
        shown: bool,
        width: f32,
        rem: f32,
        clicks: Rc<Cell<usize>>,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            div()
                .v_flex()
                .w(px(self.width))
                .h(px(900.))
                .gap(px(self.rem * 0.25))
                .child(div().h(px(65.)).flex_shrink_0())
                .child(
                    div()
                        .debug_selector(|| "artwork-scroll".into())
                        .flex_1()
                        .min_h_0(),
                )
                .child(ArtworkPresence {
                    shown: self.shown,
                    width: px(self.width),
                    distance: px(200.),
                    content: div()
                        .id("artwork-child")
                        .debug_selector(|| "artwork-child".into())
                        .role(Role::Button)
                        .aria_label("Artwork")
                        .size(px(self.width))
                        .bg(rgb(0x123456))
                        .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                        .into_any_element(),
                })
        }
    }

    #[gpui::test]
    fn artwork_presence_layout_opacity_and_pointer_targets_match_original(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../assets/sidebar-artwork-motion-reference.json"
        ))
        .unwrap();
        for case in cases {
            let clicks = Rc::new(Cell::new(0));
            let probe = cx.new(|_| Probe {
                shown: case["initial"].as_bool().unwrap(),
                width: case["config"]["width"].as_f64().unwrap() as f32,
                rem: case["config"]["rem"].as_f64().unwrap() as f32,
                clicks: clicks.clone(),
            });
            let (_, window_cx) = cx.add_window_view(|window, cx| {
                window.activate_accessibility_for_test();
                Root::new(probe.clone(), window, cx)
            });
            window_cx.simulate_resize(size(px(1000.), px(1100.)));
            window_cx.run_until_parked();
            if case["settled"] == true {
                window_cx
                    .executor()
                    .advance_clock(Duration::from_millis(400));
                window_cx.update(|window, _| window.refresh());
                window_cx.run_until_parked();
            }
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
                            probe.shown = update[1].as_bool().unwrap();
                            cx.notify();
                        });
                    }
                }
                window_cx.update(|window, _| window.refresh());
                window_cx.run_until_parked();
                let retained = window_cx.update(|window, _| {
                    window
                        .accessibility_tree_for_test()
                        .unwrap()
                        .nodes
                        .iter()
                        .any(|(_, node)| node.label() == Some("Artwork"))
                });
                assert_eq!(
                    retained,
                    frame["retained"] == true,
                    "presence at {milliseconds}: {case}"
                );
                let mut check = |selector: &str, expected: &serde_json::Value| {
                    let bounds = window_cx.debug_bounds(selector).unwrap();
                    for (actual, expected) in [
                        bounds.origin.x,
                        bounds.origin.y,
                        bounds.size.width,
                        bounds.size.height,
                    ]
                    .into_iter()
                    .zip(expected.as_array().unwrap())
                    {
                        assert!(
                            (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                            "{selector} at {milliseconds}: {bounds:?}, expected {expected}; {case}"
                        );
                    }
                    bounds
                };
                check("artwork-scroll", &frame["scroll"]);
                if retained {
                    let bounds = check("artwork-child", &frame["bounds"]);
                    let expected = frame["opacity"].as_f64().unwrap() as f32;
                    let opacity = window_cx.update(|window, _| {
                        window
                            .painted_quad_opacities_for_test()
                            .into_iter()
                            .find(|(painted, _)| {
                                (painted.origin - bounds.origin).magnitude() < 1.
                                    && (painted.size.width - bounds.size.width).abs() < px(1.)
                            })
                            .map_or(0., |(_, color)| color.a)
                    });
                    assert!(
                        (opacity - expected).abs() <= 0.001,
                        "paint at {milliseconds}: {opacity}, expected {expected}; {case}"
                    );
                    let before = clicks.get();
                    window_cx.simulate_click(
                        point(bounds.center().x, bounds.top() + px(20.)),
                        Modifiers::none(),
                    );
                    window_cx.run_until_parked();
                    assert_eq!(
                        clicks.get() > before,
                        frame["pointer"] == true,
                        "pointer at {milliseconds}: {case}"
                    );
                }
            }
            window_cx.update(|window, _| window.remove_window());
        }
    }
}
