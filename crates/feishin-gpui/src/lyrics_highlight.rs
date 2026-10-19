use gpui::{prelude::*, *};
use gpui_component::animation::CssTransition;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
struct Gradient {
    start: f32,
    end: f32,
}
impl Default for Gradient {
    fn default() -> Self {
        Self {
            start: -0.2,
            end: -0.1,
        }
    }
}
struct State {
    animating_part: bool,
    gradient: Gradient,
    opacity: CssTransition,
}
impl Default for State {
    fn default() -> Self {
        Self {
            animating_part: false,
            gradient: Gradient::default(),
            opacity: CssTransition::new(0., Duration::from_millis(500), [0.25, 0.1, 0.25, 1.]),
        }
    }
}
impl State {
    fn frame(
        &mut self,
        position: f64,
        start: f64,
        duration: f64,
        playing: bool,
        now: Instant,
    ) -> (Gradient, f32, bool) {
        let elapsed = position + 0.15 - start;
        let sung = elapsed >= duration;
        let visible = elapsed >= 0. && playing;
        if visible {
            self.opacity = CssTransition::new(1., Duration::ZERO, [0.25, 0.1, 0.25, 1.]);
            self.gradient = if duration <= 0.
                || sung && (!self.animating_part || elapsed >= duration * 1.6 || !playing)
            {
                Gradient {
                    start: 1.4,
                    end: 1.5,
                }
            } else {
                let end = ((elapsed / duration) as f32).min(1.5);
                Gradient {
                    start: end - 0.1,
                    end,
                }
            };
            self.animating_part =
                playing && (!sung || self.animating_part && elapsed < duration * 1.6);
        } else {
            if self.opacity.target() != 0. {
                self.opacity = CssTransition::new(
                    self.opacity.value(now).0,
                    Duration::from_millis(500),
                    [0.25, 0.1, 0.25, 1.],
                );
                self.opacity.retarget(0., now);
            }
            self.animating_part = false;
        }
        let (opacity, animating) = self.opacity.value(now);
        (self.gradient, opacity, animating)
    }
}

/// The original stacked text twin, painted through a soft horizontal gradient.
#[derive(IntoElement)]
pub(crate) struct Word {
    pub key: SharedString,
    pub text: String,
    pub position: f64,
    pub start: f64,
    pub duration: f64,
    pub playing: bool,
    pub base: Hsla,
    pub foreground: Hsla,
    pub zoom: f32,
}
impl RenderOnce for Word {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let rtl = feishin_core::lyrics::word_is_rtl(&self.text);
        let state = window.use_keyed_state(
            SharedString::from(format!("lyric-highlight-{}", self.key)),
            cx,
            |_, _| State::default(),
        );
        let (gradient, opacity, animating) = state.update(cx, |state, cx| {
            state.frame(
                self.position,
                self.start,
                self.duration,
                self.playing,
                cx.background_executor().now(),
            )
        });
        if animating {
            window.request_animation_frame();
        }
        let (base, layouts) = crate::lyrics_markup::karaoke_content(&self.text, window, cx);
        let key = self.key.clone();
        div()
            .id(SharedString::from(format!("lyric-word-{}", self.key)))
            .debug_selector(move || format!("lyric-word-progress-{key}"))
            .relative()
            .flex_shrink_0()
            .min_w_0()
            .max_w_full()
            .text_wrap(TextWrap::BreakWord)
            .child(div().min_w_0().w_full().text_color(self.base).child(base))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .opacity(opacity)
                    .debug_selector(move || format!("lyric-word-mask-{}", self.key))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, cx| {
                                paint_gradient(
                                    &layouts,
                                    bounds,
                                    (gradient, rtl, self.zoom, self.foreground),
                                    window,
                                    cx,
                                );
                            },
                        )
                        .size_full(),
                    ),
            )
    }
}
fn paint_gradient(
    layouts: &[TextLayout],
    bounds: Bounds<Pixels>,
    style: (Gradient, bool, f32, Hsla),
    window: &mut Window,
    cx: &mut App,
) {
    let (gradient, rtl, zoom, color) = style;
    let width = f32::from(bounds.size.width);
    let start = gradient.start * width;
    let end = gradient.end * width + zoom;
    let left = start.clamp(0., width);
    let right = end.clamp(0., width);
    let clip = |from: f32, to: f32, opacity: f32, window: &mut Window, cx: &mut App| {
        if to <= from || opacity <= 0. {
            return;
        }
        let x = if rtl { width - to } else { from };
        let region = Bounds::new(
            point(bounds.left() + px(x), bounds.top()),
            size(px(to - from), bounds.size.height),
        );
        window.with_content_mask(Some(ContentMask { bounds: region }), |window| {
            for layout in layouts {
                layout.paint_with_color(color.opacity(opacity), window, cx);
            }
        });
    };
    clip(0., left, 1., window, cx);
    // ponytail: reuse the shaped twin for each physical-pixel edge column.
    // Very large cues can justify a GPU gradient mask if profiling exceeds the frame budget.
    let step = 1. / window.scale_factor();
    let mut from = left;
    while from < right {
        let to = (from + step).min(right);
        let opacity = ((end - (from + to) / 2.) / (end - start)).clamp(0., 1.);
        clip(from, to, opacity, window, cx);
        from = to;
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use std::time::{Duration, Instant};
    #[gpui::test]
    fn native_word_twins_paint_soft_edges_in_both_directions_and_fade_on_pause(
        cx: &mut gpui::TestAppContext,
    ) {
        use super::Word;
        use gpui::{AppContext, Context, Render, Window, div, prelude::*, px};
        use gpui_component::{ActiveTheme, Root};
        use std::{cell::RefCell, rc::Rc};
        struct Probe {
            text: String,
            playing: bool,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
                let mut base = cx.theme().foreground;
                base.a = 0.2;
                div()
                    .flex()
                    .w(px(200.))
                    .text_size(px(24.))
                    .font_family("Inter")
                    .child(Word {
                        key: "probe".into(),
                        text: self.text.clone(),
                        position: 1.25,
                        start: 1.,
                        duration: 1.,
                        playing: self.playing,
                        base,
                        foreground: cx.theme().foreground,
                        zoom: 1.,
                    })
            }
        }
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let probe = cx.new(|_| Probe {
                text: "Hello".into(),
                playing: true,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        for text in ["Hello", "שלום", "اللغة"] {
            cx.update(|_, cx| {
                probe.update(cx, |probe, cx| {
                    probe.text = text.into();
                    cx.notify();
                })
            });
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            let word = cx.debug_bounds("lyric-word-progress-probe").unwrap();
            let rtl = feishin_core::lyrics::word_is_rtl(text);
            cx.update(|window, cx| {
                let sprites = window.painted_glyphs_for_test();
                let width = f32::from(word.size.width);
                let start = width * 0.3;
                let end = width * 0.4 + 1.;
                let edges = sprites
                    .iter()
                    .filter(|(_, clip, _)| {
                        f32::from(clip.size.width) <= 1. / window.scale_factor() + 0.01
                    })
                    .collect::<Vec<_>>();
                assert!(!edges.is_empty(), "No gradient columns painted for {text}");
                for (glyph, clip, color) in edges {
                    let x = f32::from(if rtl {
                        word.right() - clip.center().x
                    } else {
                        clip.center().x - word.left()
                    });
                    let expected =
                        ((end - x) / (end - start)).clamp(0., 1.) * cx.theme().foreground.a;
                    assert!(
                        (color.a - expected).abs() < 0.001,
                        "{text}: {clip:?}, alpha={}, expected={expected}",
                        color.a
                    );
                    assert!(
                        sprites.iter().any(
                            |(base, _, color)| base == glyph && (color.a - 0.2).abs() < 0.00001
                        ),
                        "Gradient changed the shaped twin"
                    );
                }
            });
        }
        cx.update(|_, cx| {
            probe.update(cx, |probe, cx| {
                probe.playing = false;
                cx.notify();
            })
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let word = cx.debug_bounds("lyric-word-progress-probe").unwrap();
        let max = cx.update(|window, _| {
            window
                .painted_glyphs_with_opacity_for_test()
                .into_iter()
                .filter(|(_, clip, _)| clip.size.width < word.size.width)
                .map(|(_, _, color)| color.a)
                .fold(0_f32, f32::max)
        });
        assert!((max - 0.197597).abs() < 0.001, "Pause opacity={max}");
    }

    #[test]
    fn word_highlights_match_original_gradient_states_and_pause_frames() {
        #[cfg(target_os = "macos")]
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/lyrics-word-motion-darwin-reference.json"
        ))
        .unwrap();
        #[cfg(not(target_os = "macos"))]
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/lyrics-word-motion-linux-reference.json"
        ))
        .unwrap();
        let now = Instant::now();
        for case in reference["cases"].as_array().unwrap() {
            for word in case["words"].as_array().unwrap() {
                let position = 1. + case["elapsed"].as_f64().unwrap() / 1000.;
                let (gradient, opacity, _) = State::default().frame(
                    position,
                    word["time"].as_f64().unwrap(),
                    word["duration"].as_f64().unwrap(),
                    case["playing"] == true,
                    now,
                );
                assert!(
                    (gradient.start - word["start"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{case}: {word}, {gradient:?}"
                );
                assert!(
                    (gradient.end - word["end"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{case}: {word}, {gradient:?}"
                );
                assert!(
                    (opacity - word["opacity"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{case}: {word}, opacity={opacity}"
                );
            }
        }
        for pause in reference["pauses"].as_array().unwrap() {
            let mut state = State::default();
            let elapsed = pause["elapsed"].as_f64().unwrap() / 1000.;
            state.frame(1. + elapsed, 1., 2., true, now);
            for frame in pause["frames"].as_array().unwrap() {
                let (gradient, opacity, _) = state.frame(
                    1.,
                    1.,
                    2.,
                    false,
                    now + Duration::from_millis(frame["milliseconds"].as_u64().unwrap()),
                );
                let word = &frame["words"][0];
                assert!(
                    (gradient.start - word["start"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{pause}: {frame}"
                );
                assert!(
                    (gradient.end - word["end"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{pause}: {frame}"
                );
                assert!(
                    (opacity - word["opacity"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{pause}: {frame}"
                );
            }
        }
        for timeline in reference["timelines"].as_array().unwrap() {
            let mut state = State::default();
            for frame in timeline["frames"].as_array().unwrap() {
                let (gradient, opacity, _) = state.frame(
                    1. + frame["elapsed"].as_f64().unwrap() / 1000.,
                    1.,
                    2.,
                    frame["playing"] == true,
                    now + Duration::from_millis(frame["milliseconds"].as_u64().unwrap()),
                );
                let word = &frame["words"][0];
                assert!(
                    (gradient.start - word["start"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{timeline}: {frame}, {gradient:?}"
                );
                assert!(
                    (gradient.end - word["end"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{timeline}: {frame}, {gradient:?}"
                );
                assert!(
                    (opacity - word["opacity"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{timeline}: {frame}, opacity={opacity}"
                );
            }
        }
    }
}
