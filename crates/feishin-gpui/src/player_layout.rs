use gpui::{prelude::*, *};
use gpui_component::StyledExt;
use gpui_component::animation::CssTransition;
use std::time::{Duration, Instant};

/// The expanded player stays mounted while Motion's exit runs.
pub struct OverlayMotion {
    open: bool,
    mounted: bool,
    slide: CssTransition,
    color: CssTransition,
    from: Rgba,
    to: Rgba,
    exit_at: Option<Instant>,
}
impl OverlayMotion {
    pub fn new(open: bool, color: Rgba) -> Self {
        let color = if open {
            color
        } else {
            transparent_black().into()
        };
        Self {
            open,
            mounted: open,
            slide: Self::transition(if open { 0. } else { 1. }),
            color: Self::transition(1.),
            from: color,
            to: color,
            exit_at: None,
        }
    }
    fn transition(value: f32) -> CssTransition {
        CssTransition::new(value, Duration::from_millis(500), [0., 0., 0.58, 1.])
    }
    fn color_at(&self, now: Instant) -> Rgba {
        let t = self.color.value(now).0;
        // Motion mixes squared sRGB channels, rounding each CSS channel to an integer.
        let channel = |from: f32, to: f32| {
            ((from * from + (to * to - from * from) * t).max(0.).sqrt() * 255.).round() / 255.
        };
        Rgba {
            r: channel(self.from.r, self.to.r),
            g: channel(self.from.g, self.to.g),
            b: channel(self.from.b, self.to.b),
            a: self.from.a + (self.to.a - self.from.a) * t,
        }
    }
    pub fn frame(&mut self, open: bool, color: Rgba, now: Instant) -> (f32, Rgba, bool, bool) {
        if self.exit_at.is_some_and(|at| now >= at) {
            self.mounted = false;
            self.exit_at = None;
        }
        if open != self.open {
            if open && !self.mounted {
                self.slide = Self::transition(1.);
                self.from = transparent_black().into();
                self.to = self.from;
                self.color = Self::transition(1.);
            }
            self.open = open;
            self.mounted = true;
            self.exit_at = (!open).then_some(now + Duration::from_millis(500));
            self.slide = Self::transition(self.slide.value(now).0);
            self.slide.retarget(
                if open { 0. } else { 1. },
                now + Duration::from_millis(if open { 100 } else { 0 }),
            );
        }
        if open && color != self.to {
            self.from = self.color_at(now);
            self.to = color;
            self.color = Self::transition(0.);
            self.color.retarget(1., now + Duration::from_millis(100));
        }
        let (slide, sliding) = self.slide.value(now);
        (
            slide,
            self.color_at(now),
            self.mounted,
            self.mounted && (sliding || self.color.value(now).1 || self.exit_at.is_some()),
        )
    }
}

#[derive(IntoElement)]
pub struct ScrollingTitle {
    pub title: String,
    pub font_size: Pixels,
    pub zoom: f32,
}
impl RenderOnce for ScrollingTitle {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let timing = window.use_keyed_state(
            SharedString::from(format!("player-title-clock-{}", self.title)),
            cx,
            |_, cx| (None::<(Pixels, Pixels)>, cx.background_executor().now()),
        );
        div()
            .id("player-title")
            .debug_selector(|| "player-title-scroll".into())
            .role(Role::Paragraph)
            .aria_label(self.title.clone())
            .w_full()
            .h(self.font_size * 1.2)
            .font_weight(FontWeight::BLACK)
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        let text: SharedString = self.title.into();
                        let run = TextRun {
                            letter_spacing_fraction: 0.,
                            len: text.len(),
                            font: window.text_style().font(),
                            color: window.text_style().color,
                            font_size: None,
                            line_height: None,
                            background_color: None,
                            underline: None,
                            overline: None,
                            strikethrough: None,
                            letter_spacing: px(0.),
                            text_spacing_trim: window.text_style().text_spacing_trim,
                            text_direction: window.text_style().text_direction,
                        };
                        let line =
                            window
                                .text_system()
                                .shape_line(text, self.font_size, &[run], None);
                        let now = cx.background_executor().now();
                        let elapsed = timing.update(cx, |timing, _| {
                            let dimensions = (line.width, bounds.size.width);
                            if timing.0 != Some(dimensions) {
                                *timing = (Some(dimensions), now);
                            }
                            now.saturating_duration_since(timing.1).as_secs_f32()
                        });
                        let overflow = line.width > bounds.size.width;
                        let distance = line.width + px(100. * self.zoom);
                        let offset = if overflow {
                            px(title_offset(elapsed, distance.into(), 50. * self.zoom))
                        } else {
                            (line.width - bounds.size.width) / 2.
                        };
                        window.with_content_mask(Some(ContentMask { bounds }), |window| {
                            let position = bounds.origin - point(offset, px(0.));
                            let _ = line.paint(position, self.font_size * 1.2, window, cx);
                            if overflow {
                                let _ = line.paint(
                                    position + point(distance, px(0.)),
                                    self.font_size * 1.2,
                                    window,
                                    cx,
                                );
                            }
                        });
                        if overflow {
                            window.request_animation_frame();
                        }
                    },
                )
                .size_full(),
            )
    }
}
fn title_offset(elapsed: f32, distance: f32, speed: f32) -> f32 {
    let time = elapsed % (5. + distance / speed);
    (time - 5.).max(0.) * speed
}

pub struct ArtworkLayer {
    pub key: String,
    pub item: feishin_core::model::Item,
    opacity: CssTransition,
    exit_at: Option<Instant>,
    ready: bool,
}
pub struct ArtworkMotion {
    layers: Vec<ArtworkLayer>,
}
impl ArtworkMotion {
    pub fn new(key: &str, item: &feishin_core::model::Item, ready: bool) -> Self {
        Self {
            layers: if key.is_empty() {
                Vec::new()
            } else {
                vec![Self::layer(key, item, 1., ready)]
            },
        }
    }
    fn layer(
        key: &str,
        item: &feishin_core::model::Item,
        opacity: f32,
        ready: bool,
    ) -> ArtworkLayer {
        ArtworkLayer {
            key: key.into(),
            item: item.clone(),
            opacity: CssTransition::new(opacity, Duration::from_millis(400), [0., 0., 1., 1.]),
            exit_at: None,
            ready,
        }
    }
    pub fn frame(
        &mut self,
        key: &str,
        item: &feishin_core::model::Item,
        ready: bool,
        now: Instant,
    ) -> (Vec<(String, feishin_core::model::Item, f32)>, bool) {
        // AnimatePresence removes a group of exiting children together.
        if self
            .layers
            .iter()
            .filter_map(|layer| layer.exit_at)
            .all(|at| now >= at)
        {
            self.layers.retain(|layer| layer.exit_at.is_none());
        }
        for layer in &mut self.layers {
            let present = layer.key == key;
            if present {
                layer.item = item.clone();
                if ready && !layer.ready {
                    layer.opacity =
                        CssTransition::new(0., Duration::from_millis(400), [0., 0., 1., 1.]);
                    layer.opacity.retarget(1., now);
                }
                layer.ready = ready;
            }
            if present == layer.exit_at.is_some() {
                let duration = Duration::from_millis(if !layer.ready {
                    0
                } else if present {
                    400
                } else {
                    800
                });
                layer.opacity =
                    CssTransition::new(layer.opacity.value(now).0, duration, [0., 0., 1., 1.]);
                layer.opacity.retarget(f32::from(present), now);
                layer.exit_at = (!present).then_some(now + duration);
            }
        }
        if !key.is_empty() && !self.layers.iter().any(|layer| layer.key == key) {
            let mut layer = Self::layer(key, item, if ready { 0. } else { 1. }, ready);
            layer.opacity.retarget(1., now);
            self.layers.push(layer);
        }
        if self
            .layers
            .iter()
            .filter_map(|layer| layer.exit_at)
            .all(|at| now >= at)
        {
            self.layers.retain(|layer| layer.exit_at.is_none());
        }
        let mut active = false;
        let layers = self
            .layers
            .iter()
            .map(|layer| {
                let (opacity, animating) = layer.opacity.value(now);
                active |= animating || layer.exit_at.is_some();
                (layer.key.clone(), layer.item.clone(), opacity)
            })
            .collect();
        (layers, active)
    }
}

/// Motion's wait mode keeps the outgoing panel mounted until its exit completes.
pub struct PanelMotion {
    tab: String,
    opacity: CssTransition,
    exit_at: Option<Instant>,
}
impl PanelMotion {
    pub fn new(tab: &str) -> Self {
        Self {
            tab: tab.into(),
            opacity: CssTransition::new(1., Duration::from_millis(400), [0., 0., 0.58, 1.]),
            exit_at: None,
        }
    }
    pub fn frame(&mut self, tab: &str, now: Instant) -> (String, f32, bool) {
        if self.tab == tab {
            if self.exit_at.take().is_some() {
                self.tween(1., now);
            }
        } else if self.tab.is_empty() {
            self.tab = tab.into();
            self.opacity = CssTransition::new(0., Duration::from_millis(400), [0., 0., 0.58, 1.]);
            self.opacity.retarget(1., now);
        } else if let Some(deadline) = self.exit_at {
            if now >= deadline {
                self.tab = tab.into();
                self.exit_at = None;
                self.opacity =
                    CssTransition::new(0., Duration::from_millis(400), [0., 0., 0.58, 1.]);
                self.opacity.retarget(1., deadline);
            }
        } else {
            self.tween(0., now);
            self.exit_at = Some(now + Duration::from_millis(400));
        }
        let (opacity, active) = self.opacity.value(now);
        (self.tab.clone(), opacity, active || self.exit_at.is_some())
    }
    fn tween(&mut self, target: f32, now: Instant) {
        self.opacity = CssTransition::new(
            self.opacity.value(now).0,
            Duration::from_millis(400),
            [0., 0., 0.58, 1.],
        );
        self.opacity.retarget(target, now);
    }
}

struct Motion {
    image: CssTransition,
    panel: CssTransition,
}
impl Motion {
    fn new(open: bool) -> Self {
        Self {
            image: CssTransition::new(
                if open { 0.5 } else { 1. },
                Duration::from_millis(500),
                [0., 0., 0.58, 1.],
            ),
            panel: CssTransition::new(
                if open { 0. } else { 1. },
                Duration::from_millis(500),
                [0., 0., 0.58, 1.],
            ),
        }
    }
    fn frame(&mut self, open: bool, now: std::time::Instant) -> (f32, f32, bool) {
        self.image.retarget_with_timing(
            if open { 0.5 } else { 1. },
            Duration::from_millis(if open { 500 } else { 600 }),
            if open {
                [0., 0., 0.58, 1.]
            } else {
                [0.34, 1.38, 0.64, 1.]
            },
            now,
        );
        self.panel.retarget(if open { 0. } else { 1. }, now);
        let (image, a) = self.image.value(now);
        let (panel, b) = self.panel.value(now);
        (image, panel, a || b)
    }
}

#[derive(IntoElement)]
pub struct PlayerLayout {
    pub open: bool,
    pub portrait: bool,
    pub font: Pixels,
    pub zoom: f32,
    pub image: AnyElement,
    pub panel: AnyElement,
}
impl RenderOnce for PlayerLayout {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state =
            window.use_keyed_state("expanded-player-layout", cx, |_, _| Motion::new(self.open));
        let (image, panel, animating) = state.update(cx, |state, cx| {
            state.frame(self.open, cx.background_executor().now())
        });
        if animating {
            window.request_animation_frame();
        }
        div()
            .debug_selector(|| "expanded-player-layout".into())
            .relative()
            .flex()
            .justify_center()
            .size_full()
            .overflow_hidden()
            .when(self.portrait, |div| {
                div.px(self.font * 2.).pt(self.font * 2.).pb(self.font)
            })
            .child(
                div()
                    .debug_selector(|| "expanded-player-responsive".into())
                    .relative()
                    .w_full()
                    .max_w(px(2560. * self.zoom))
                    .my(self.font * 2.)
                    .when(self.portrait, |div| div.mt_0())
                    .map(|div| {
                        crate::styles::apply(
                            div,
                            "div",
                            "",
                            &["fs-full-screen-player-module-responsive-container"],
                            Some(feishin_core::styles::Target::new(
                                "div",
                                "expanded-player",
                                &["fs-full-screen-player-module-container"],
                            )),
                            cx,
                        )
                    })
                    .child(
                        div()
                            .debug_selector(|| "expanded-player-image-column".into())
                            .absolute()
                            .top_0()
                            .left_0()
                            .map(|div| {
                                if self.portrait {
                                    div.w_full().h(relative(image))
                                } else {
                                    div.w(relative(image)).h_full()
                                }
                            })
                            .child(self.image),
                    )
                    .child(
                        div()
                            .debug_selector(|| "expanded-player-panel-column".into())
                            .absolute()
                            .p(self.font)
                            .overflow_hidden()
                            .map(|div| {
                                if self.portrait {
                                    div.left_0()
                                        .top(relative(0.5 + panel * 0.5))
                                        .w_full()
                                        .h(relative(0.5))
                                } else {
                                    div.left(relative(0.5 + panel * 0.5))
                                        .top_0()
                                        .w(relative(0.5))
                                        .h_full()
                                }
                            })
                            .map(|div| {
                                crate::styles::apply(
                                    div,
                                    "div",
                                    "",
                                    &["fs-full-screen-player-queue-module-grid-container"],
                                    None,
                                    cx,
                                )
                            })
                            .child(crate::styles::apply_in(
                                div()
                                    .debug_selector(|| "expanded-player-panel-background".into())
                                    .absolute()
                                    .inset_0()
                                    .rounded(px(5. * self.zoom)),
                                feishin_core::styles::Target::pseudo("before"),
                                [feishin_core::styles::Target::new(
                                    "div",
                                    "",
                                    &["fs-full-screen-player-queue-module-grid-container"],
                                )],
                                cx,
                            ))
                            .child(self.panel),
                    ),
            )
    }
}

#[derive(IntoElement)]
pub struct ArtworkLayout {
    pub font: Pixels,
    pub cover: f32,
    pub show_metadata: bool,
    pub image: AnyElement,
    pub metadata: AnyElement,
    pub measured_width: Option<Entity<Option<Pixels>>>,
}

/// The image keeps its specified height when max-width constrains its intrinsic width.
pub fn artwork_image<T: Styled + IntoElement + 'static>(
    image: T,
    native_aspect: Option<f32>,
    zoom: f32,
    explicit: bool,
) -> AnyElement {
    canvas(
        move |bounds, window, cx| {
            let width = native_aspect.map_or(bounds.size.width, |aspect| {
                bounds.size.width.min(bounds.size.height * aspect)
            });
            let filter = if explicit {
                BackdropFilterOperation::Blur(30. * zoom)
            } else {
                BackdropFilterOperation::DropShadow {
                    offset: [0., 5. * zoom],
                    sigma: 15. * zoom,
                    color: Some([0., 0., 0., 0.35]),
                }
            };
            let mut image = crate::styles::apply(
                image,
                "img",
                "",
                if explicit {
                    &[
                        "fs-full-screen-player-image-module-image",
                        "full-screen-player-image",
                        "fs-full-screen-player-image-module-censored",
                    ]
                } else {
                    &[
                        "fs-full-screen-player-image-module-image",
                        "full-screen-player-image",
                    ]
                },
                None,
                cx,
            );
            // Filters replace the original shadow and opacity applies to the
            // filtered result, including any shadow pixels beyond the image.
            let filters = image
                .style()
                .foreground_filters
                .take()
                .unwrap_or_else(|| vec![filter]);
            let opacity = image.style().opacity.take().unwrap_or(1.);
            let mut image = div()
                .debug_selector(|| "expanded-player-image-frame".into())
                .w(width)
                .h(bounds.size.height)
                .foreground_filters(filters)
                .opacity(opacity)
                .child(image)
                .into_any_element();
            image.layout_as_root(
                size(
                    AvailableSpace::Definite(width),
                    AvailableSpace::Definite(bounds.size.height),
                ),
                window,
                cx,
            );
            image.prepaint_at(
                bounds.origin + point((bounds.size.width - width) / 2., px(0.)),
                window,
                cx,
            );
            image
        },
        |_, mut image, window, cx| image.paint(window, cx),
    )
    .size_full()
    .into_any_element()
}
impl RenderOnce for ArtworkLayout {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .debug_selector(|| "expanded-player-artwork-layout".into())
            .v_flex()
            .items_center()
            .justify_center()
            .size_full()
            .p(self.font)
            .child(
                canvas(
                    move |bounds, window, cx| {
                        // Chromium resolves the aspect-ratio width from the final,
                        // flex-shrunk height. Taffy resolves it before max-height.
                        let width = bounds.size.width.min(bounds.size.height);
                        if let Some(measured) = self.measured_width {
                            measured.update(cx, |measured, cx| {
                                if measured.is_none() {
                                    *measured = Some(width);
                                    cx.notify();
                                }
                            });
                        }
                        let mut image = div()
                            .debug_selector(|| "player-artwork".into())
                            .relative()
                            .flex()
                            .items_end()
                            .justify_center()
                            .w(width)
                            .h(bounds.size.height)
                            .child(self.image)
                            .into_any_element();
                        image.layout_as_root(
                            size(
                                AvailableSpace::Definite(width),
                                AvailableSpace::Definite(bounds.size.height),
                            ),
                            window,
                            cx,
                        );
                        image.prepaint_at(
                            bounds.origin + point((bounds.size.width - width) / 2., px(0.)),
                            window,
                            cx,
                        );
                        image
                    },
                    |_, mut image, window, cx| image.paint(window, cx),
                )
                .w_full()
                .h_full()
                .min_h_0()
                .max_h(relative(self.cover / 100.))
                .when(self.show_metadata, |image| image.mb(self.font * 2.)),
            )
            .child(
                div()
                    .debug_selector(|| "expanded-player-metadata-layout".into())
                    .flex_shrink_0()
                    .w_full()
                    .child(self.metadata),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtworkLayout, ArtworkMotion, OverlayMotion, PanelMotion, PlayerLayout, artwork_image,
    };
    use gpui::{
        AppContext, Context, Render, Rgba, TestAppContext, Window, div, prelude::*, px, size,
    };
    use gpui_component::Root;
    use std::time::{Duration, Instant};

    #[test]
    fn expanded_player_overlay_and_color_match_original_motion_frames() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/full-player-background-motion-reference.json"
        ))
        .unwrap();
        let color = |state: &serde_json::Value| {
            let css = if state["dynamic"] == true {
                state["color"].as_str().unwrap()
            } else {
                "rgb(17, 25, 31)"
            };
            let parts: Vec<f32> = css
                .trim_start_matches("rgb(")
                .trim_end_matches(')')
                .split(',')
                .map(|part| part.trim().parse().unwrap())
                .collect();
            Rgba {
                r: parts[0] / 255.,
                g: parts[1] / 255.,
                b: parts[2] / 255.,
                a: 1.,
            }
        };
        for case in cases.as_array().unwrap() {
            let mut state = &case["initial"];
            let mut motion = OverlayMotion::new(state["open"] == true, color(state));
            let start = Instant::now();
            for frame in case["frames"].as_array().unwrap() {
                let ms = frame["milliseconds"].as_u64().unwrap();
                let now = start + Duration::from_millis(ms);
                motion.frame(state["open"] == true, color(state), now);
                for update in case["updates"].as_array().unwrap() {
                    if update[0].as_u64() == Some(ms) {
                        state = &update[1];
                    }
                }
                let (y, color, mounted, _) = motion.frame(state["open"] == true, color(state), now);
                let layers = frame["layers"].as_array().unwrap();
                assert_eq!(mounted, !layers.is_empty(), "{case}: {ms}");
                if !mounted {
                    continue;
                }
                assert!(
                    (y * 400. - layers[0]["y"].as_f64().unwrap() as f32).abs() < 0.4,
                    "{case}: {ms}: {y}"
                );
                let css = layers[0]["color"].as_str().unwrap();
                let channels: Vec<f32> = css
                    .split_once('(')
                    .unwrap()
                    .1
                    .trim_end_matches(')')
                    .split(',')
                    .map(|part| part.trim().parse().unwrap())
                    .collect();
                for (actual, expected) in [color.r * 255., color.g * 255., color.b * 255.]
                    .into_iter()
                    .zip(channels.iter().copied())
                {
                    assert!(
                        (actual - expected).abs() <= 1.,
                        "{case}: {ms}: {actual} != {expected}"
                    );
                }
                assert!(
                    // Chromium serializes alpha through an eight-bit channel.
                    (color.a - channels.get(3).copied().unwrap_or(1.)).abs() < 1. / 255.,
                    "{case}: {ms}: alpha {}",
                    color.a
                );
            }
        }
    }

    #[test]
    fn expanded_player_title_pause_speed_and_repeat_match_original() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/player-title-motion-reference.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let width = case["width"].as_f64().unwrap() as f32;
            let overflow = width > case["container"].as_f64().unwrap() as f32;
            for frame in case["frames"].as_array().unwrap() {
                let ms = frame["milliseconds"].as_f64().unwrap() as f32;
                let value = if overflow {
                    super::title_offset(ms / 1000., width + 100., 50.)
                } else {
                    0.
                };
                // The original filled animation holds the identical second copy
                // during the next pause, so a complete lap paints the same text.
                let expected = frame["offset"].as_f64().unwrap() as f32 % (width + 100.);
                assert!((value - expected).abs() < 0.01, "{case}: {ms}: {value}");
            }
        }
    }

    #[test]
    fn expanded_player_artwork_crossfades_match_original_motion_frames() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/full-player-motion-reference.json"))
                .unwrap();
        let item = feishin_core::model::Item::default();
        for case in cases
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["kind"] == "artwork")
        {
            let mut key = case["initial"].as_str().unwrap();
            let mut motion = ArtworkMotion::new(key, &item, true);
            let start = std::time::Instant::now();
            for frame in case["frames"].as_array().unwrap() {
                let ms = frame["milliseconds"].as_u64().unwrap();
                let now = start + Duration::from_millis(ms);
                motion.frame(key, &item, true, now);
                for update in case["updates"].as_array().unwrap() {
                    if update[0].as_u64() == Some(ms) {
                        key = update[1].as_str().unwrap();
                    }
                }
                let (layers, _) = motion.frame(key, &item, true, now);
                let expected = frame["layers"].as_array().unwrap();
                assert_eq!(layers.len(), expected.len(), "{case}: {ms}");
                for ((key, _, opacity), layer) in layers.iter().zip(expected) {
                    assert_eq!(key, layer["key"].as_str().unwrap(), "{case}: {ms}");
                    assert!(
                        (opacity - layer["opacity"].as_f64().unwrap() as f32).abs() < 0.001,
                        "{case}: {ms}: {opacity}"
                    );
                }
            }
        }
    }

    struct ArtworkProbe {
        cover: f32,
        metadata: f32,
        aspect: f32,
    }
    impl Render for ArtworkProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            ArtworkLayout {
                font: px(16.),
                cover: self.cover,
                show_metadata: self.metadata > 0.,
                image: artwork_image(div().size_full(), Some(self.aspect), 1., false),
                metadata: div().h(px(self.metadata)).into_any_element(),
                measured_width: None,
            }
        }
    }
    #[gpui::test]
    fn expanded_player_artwork_size_matches_original_flex_constraints(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/full-player-layout-reference.json"))
                .unwrap();
        for case in reference["artwork"].as_array().unwrap().iter() {
            let probe = cx.new(|_| ArtworkProbe {
                cover: case["cover"].as_f64().unwrap() as f32,
                metadata: case["metadata"]["height"].as_f64().unwrap() as f32,
                aspect: case["aspect"].as_f64().unwrap() as f32,
            });
            let (_, cx) = cx.add_window_view(|window, cx| Root::new(probe, window, cx));
            cx.simulate_resize(size(
                px(case["width"].as_f64().unwrap() as f32),
                px(case["height"].as_f64().unwrap() as f32),
            ));
            cx.run_until_parked();
            for (name, selector) in [
                ("container", "player-artwork"),
                ("metadata", "expanded-player-metadata-layout"),
                ("image", "expanded-player-image-frame"),
            ] {
                let bounds = cx.debug_bounds(selector).unwrap();
                for (label, value) in [
                    ("x", bounds.origin.x),
                    ("y", bounds.origin.y),
                    ("width", bounds.size.width),
                    ("height", bounds.size.height),
                ] {
                    assert!(
                        (f32::from(value) - case[name][label].as_f64().unwrap() as f32).abs() <= 1.,
                        "{case}: {name}.{label}={value:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn expanded_player_panel_presence_matches_original_motion_frames() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/full-player-motion-reference.json"))
                .unwrap();
        for case in cases
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["kind"] == "panel")
        {
            let mut tab = case["initial"].as_str().unwrap();
            let mut motion = PanelMotion::new(tab);
            let start = std::time::Instant::now();
            for frame in case["frames"].as_array().unwrap() {
                let ms = frame["milliseconds"].as_u64().unwrap();
                let now = start + Duration::from_millis(ms);
                // React first resolves running animations, then applies the user's update.
                motion.frame(tab, now);
                for update in case["updates"].as_array().unwrap() {
                    if update[0].as_u64() == Some(ms) {
                        tab = update[1].as_str().unwrap();
                    }
                }
                let (rendered, opacity, _) = motion.frame(tab, now);
                let layers = frame["layers"].as_array().unwrap();
                if rendered.is_empty() {
                    assert!(layers.is_empty(), "{case}: {ms}");
                } else {
                    assert_eq!(layers.len(), 1, "{case}: {ms}");
                    assert_eq!(rendered, layers[0]["key"].as_str().unwrap(), "{case}: {ms}");
                    assert!(
                        (opacity - layers[0]["opacity"].as_f64().unwrap() as f32).abs() < 0.001,
                        "{case}: {ms}: {opacity}"
                    );
                    assert!(
                        ((1. - opacity) * 60. - layers[0]["x"].as_f64().unwrap() as f32).abs()
                            < 0.06,
                        "{case}: {ms}"
                    );
                }
            }
        }
    }

    struct Probe {
        open: bool,
        portrait: bool,
        font: f32,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            PlayerLayout {
                open: self.open,
                portrait: self.portrait,
                font: px(self.font),
                zoom: 1.,
                image: div().into_any_element(),
                panel: div().into_any_element(),
            }
        }
    }
    #[gpui::test]
    fn expanded_player_columns_match_original_responsive_layout_and_motion(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_component::init);
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/full-player-layout-reference.json"))
                .unwrap();
        for (is_motion, reversal, cases) in [
            (false, false, &reference["layouts"]),
            (false, false, &reference["themed"]),
            (true, false, &reference["motion"]),
            (true, true, &reference["reversals"]),
        ] {
            for case in cases.as_array().unwrap() {
                let width = case["width"].as_f64().unwrap() as f32;
                let height = case["height"].as_f64().unwrap() as f32;
                let font = case["font"].as_f64().unwrap_or(16.) as f32;
                let open = case["open"].as_bool().unwrap();
                let css = if case["theme"] == "glassy" {
                    include_str!("../assets/styles/glassy-dark.css")
                } else {
                    ""
                };
                cx.update(|cx| {
                    cx.set_global(crate::styles::NativeStyles::new(
                        feishin_core::styles::Stylesheet::parse([css]).unwrap(),
                        feishin_core::appearance::Palette::default(),
                    ))
                });
                let probe = cx.new(|_| Probe {
                    open: if is_motion { !open } else { open },
                    portrait: height >= width,
                    font,
                });
                let (_, cx) = cx.add_window_view(|window, cx| Root::new(probe.clone(), window, cx));
                cx.simulate_resize(size(px(width), px(height)));
                cx.run_until_parked();
                if is_motion {
                    probe.update(cx, |probe, cx| {
                        probe.open = open;
                        cx.notify();
                    });
                    cx.run_until_parked();
                }
                if reversal {
                    cx.executor().advance_clock(Duration::from_millis(
                        case["interruptAt"].as_u64().unwrap(),
                    ));
                    probe.update(cx, |probe, cx| {
                        probe.open = !open;
                        cx.notify();
                    });
                    cx.run_until_parked();
                }
                let mut previous = 0;
                let frames = if is_motion {
                    case["frames"].as_array().unwrap().clone()
                } else {
                    vec![serde_json::json!({"milliseconds":0,"layout":case})]
                };
                for frame in frames {
                    let milliseconds = frame["milliseconds"].as_u64().unwrap();
                    cx.executor()
                        .advance_clock(Duration::from_millis(milliseconds - previous));
                    previous = milliseconds;
                    cx.update(|window, _| window.refresh());
                    cx.run_until_parked();
                    for (name, selector) in [
                        ("outer", "expanded-player-layout"),
                        ("responsive", "expanded-player-responsive"),
                        ("image", "expanded-player-image-column"),
                        ("panel", "expanded-player-panel-column"),
                    ] {
                        let bounds = cx.debug_bounds(selector).unwrap();
                        let expected = &frame["layout"][name];
                        for (label, value) in [
                            ("x", bounds.origin.x),
                            ("y", bounds.origin.y),
                            ("width", bounds.size.width),
                            ("height", bounds.size.height),
                        ] {
                            assert!(
                                (f32::from(value) - expected[label].as_f64().unwrap() as f32).abs()
                                    <= 1.,
                                "{case}: frame {milliseconds} {name}.{label}: {value:?}, expected {}",
                                expected[label]
                            );
                        }
                    }
                }
            }
        }
    }
}
