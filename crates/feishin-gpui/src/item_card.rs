use gpui_component::animation::CssTransition;
use std::time::{Duration, Instant};

// Motion's default card variants use a 300 ms opacity tween and a spring for
// either vertical movement (poster) or scale (compact). Reversals keep velocity.
struct ControlsMotion {
    compact: bool,
    visible: bool,
    started: Option<Instant>,
    opacity: CssTransition,
    from: f32,
    target: f32,
    velocity: f32,
    settled: bool,
    samples: [Option<(Instant, f32)>; 2],
}
impl ControlsMotion {
    fn new(compact: bool) -> Self {
        let hidden = if compact { 0.5 } else { 10. };
        Self {
            compact,
            visible: false,
            started: None,
            opacity: CssTransition::new(0., Duration::from_millis(300), [0.25, 0.1, 0.35, 1.]),
            from: hidden,
            target: hidden,
            velocity: 0.,
            settled: true,
            samples: [None, None],
        }
    }

    fn spring(&self, millis: f32) -> (f32, bool) {
        let (stiffness, damping): (f32, f32) = if self.compact {
            (550., 30.)
        } else {
            (500., 25.)
        };
        let decay = damping / 2.;
        let angular = (stiffness - decay * decay).sqrt();
        let delta = self.target - self.from;
        let a = (-self.velocity + decay * delta) / angular;
        let seconds = millis / 1000.;
        let envelope = (-decay * seconds).exp();
        let (sin, cos) = (angular * seconds).sin_cos();
        let value = self.target - envelope * (a * sin + delta * cos);
        let velocity =
            envelope * ((decay * a + delta * angular) * sin + (decay * delta - a * angular) * cos);
        let done = velocity.abs() <= 10.
            && (self.target - value).abs() <= if delta.abs() < 5. { 0.005 } else { 0.5 };
        (if done { self.target } else { value }, done)
    }

    fn frame(&mut self, now: Instant) -> Option<(f32, f32, bool)> {
        let started = self.started?;
        let elapsed = now.saturating_duration_since(started).as_secs_f32() * 1000.;
        let (mut value, settled) = self.spring(elapsed);
        self.settled |= settled;
        if self.settled {
            value = self.target;
        }
        let settled = self.settled;
        let (opacity, animating) = self.opacity.value(now);
        if self.samples[1].is_none_or(|(time, _)| time != now) {
            self.samples[0] = self.samples[1];
        }
        self.samples[1] = Some((now, value));
        if !self.visible && settled && !animating {
            None
        } else {
            Some((opacity, value, !settled || animating))
        }
    }

    fn set_visible(&mut self, visible: bool, now: Instant) {
        if self.visible == visible {
            return;
        }
        let frame = self.frame(now);
        let (opacity, from, _) = frame.unwrap_or((0., if self.compact { 0.5 } else { 10. }, false));
        let velocity = match self.samples {
            [Some((previous_time, previous)), Some((time, value))]
                if frame.is_some()
                    && now.saturating_duration_since(time) <= Duration::from_millis(30) =>
            {
                let seconds = time
                    .saturating_duration_since(previous_time)
                    .as_secs_f32()
                    .min(0.03);
                if seconds > 0. {
                    (value - previous) / seconds
                } else {
                    0.
                }
            }
            _ => 0.,
        };
        self.visible = visible;
        self.started = Some(now);
        self.from = from;
        self.target = if self.compact {
            if visible { 1. } else { 0.5 }
        } else if visible {
            0.
        } else {
            10.
        };
        self.velocity = velocity;
        self.settled = false;
        self.opacity =
            CssTransition::new(opacity, Duration::from_millis(300), [0.25, 0.1, 0.35, 1.]);
        self.opacity.retarget(f32::from(visible), now);
    }
}

use crate::{app::Feishin, ui::CardSource};
use feishin_core::{lists::Config, model::Item};
use gpui::{prelude::*, *};
use std::{cell::RefCell, rc::Rc};

#[derive(IntoElement)]
pub(crate) struct Card {
    pub app: WeakEntity<Feishin>,
    pub item: Item,
    pub index: usize,
    pub source: CardSource,
    pub width: Option<Pixels>,
    pub config: Config,
}
pub(crate) struct State {
    motion: ControlsMotion,
    dim: CssTransition,
    pub size: Size<Pixels>,
}
impl State {
    pub fn hover(&mut self, visible: bool, now: Instant) {
        self.motion.set_visible(visible, now);
        self.dim.retarget(f32::from(visible), now);
    }
}
pub(crate) struct Frame {
    pub controls: Option<(f32, f32, bool)>,
    pub dim: f32,
    pub hovered: bool,
    pub width: Pixels,
    pub height: Pixels,
}
impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let compact = self.source == CardSource::Library
            && self.config.grid.size == feishin_core::lists::Size::Compact;
        let key: SharedString = format!(
            "card-{}-{:?}-{}-{}",
            self.item.server_id, self.item.kind, self.item.id, self.index
        )
        .into();
        let state = window
            .use_keyed_state(key, cx, |_, _| {
                let width = self.width.unwrap_or(px(self.config.grid.artwork_height()));
                Rc::new(RefCell::new(State {
                    motion: ControlsMotion::new(compact),
                    dim: CssTransition::new(0., Duration::from_millis(200), [0.42, 0., 0.58, 1.]),
                    size: size(width, width),
                }))
            })
            .read(cx)
            .clone();
        let frame = {
            let mut state = state.borrow_mut();
            let now = cx.background_executor().now();
            if state.motion.compact != compact {
                let visible = state.motion.visible;
                state.motion = ControlsMotion::new(compact);
                state.motion.set_visible(visible, now);
            }
            let controls = state.motion.frame(now);
            let (dim, moving) = state.dim.value(now);
            if moving || controls.is_some_and(|(_, _, moving)| moving) {
                window.request_animation_frame();
            }
            Frame {
                controls,
                dim,
                hovered: state.motion.visible,
                width: self.width.unwrap_or(state.size.width),
                height: state.size.height,
            }
        };
        self.app
            .update(cx, |view, cx| view.render_card(&self, state, frame, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

pub(crate) fn css_target(tag: &str, classes: &[&str]) -> feishin_core::styles::Target {
    let mut target = feishin_core::styles::Target::new(tag, "", &[]);
    target.classes = classes
        .iter()
        .map(|class| format!("fs-item-card-module-{class}"))
        .collect();
    target
}

fn css_pixels(value: Option<Length>, base: Pixels, font: Pixels, fallback: Pixels) -> Pixels {
    match value {
        Some(Length::Definite(length)) => length.to_pixels(base.into(), font),
        _ => fallback,
    }
}

pub(crate) fn favorite_badge(
    zoom: f32,
    dim: f32,
    parent_size: Size<Pixels>,
    parents: Vec<feishin_core::styles::Target>,
    cx: &App,
) -> Div {
    let font = cx.theme().font_size;
    let mut style = badge_style("favorite-badge", parents, dim, cx);
    let width = css_pixels(style.size.width, parent_size.width, font, px(80. * zoom));
    let height = css_pixels(style.size.height, parent_size.height, font, px(80. * zoom));
    let background = style
        .background
        .take()
        .and_then(|fill| fill.color())
        .unwrap_or_else(|| cx.theme().primary.into());
    let shadows = style.box_shadow.take().unwrap_or_else(|| {
        vec![BoxShadow {
            color: black().opacity(0.8),
            offset: point(px(0.), px(0.)),
            blur_radius: px(10. * zoom),
            spread_radius: px(8. * zoom),
        }]
    });
    // Rotate the original box and its shadow about their common center. Keeping
    // the box separate lets CSS size, position, opacity and shadow overrides work.
    let rotated = move |spread: Pixels, offset: Point<Pixels>, color: Background| {
        let half = size(width / 2. + spread, height / 2. + spread);
        let scale = std::f32::consts::FRAC_1_SQRT_2;
        let extent = (half.width + half.height) * scale;
        let offset = point((offset.x + offset.y) * scale, (offset.y - offset.x) * scale);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let mut path = PathBuilder::fill();
                for (index, (x, y)) in [
                    (-half.width, -half.height),
                    (half.width, -half.height),
                    (half.width, half.height),
                    (-half.width, half.height),
                ]
                .into_iter()
                .enumerate()
                {
                    let point = bounds.center() + point((x + y) * scale, (y - x) * scale);
                    if index == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            },
        )
        .absolute()
        .left(width / 2. - extent + offset.x)
        .top(height / 2. - extent + offset.y)
        .size(extent * 2.)
    };
    div()
        .absolute()
        .left(px(-50. * zoom))
        .top(px(-50. * zoom))
        .w(width)
        .h(height)
        .refine_style(&style)
        .children(shadows.into_iter().rev().map(|shadow| {
            rotated(shadow.spread_radius, shadow.offset, shadow.color.into())
                .foreground_blur(shadow.blur_radius / 2.)
        }))
        .child(rotated(px(0.), point(px(0.), px(0.)), background))
}

fn control_target(tag: &str, name: &str, classes: &[&str]) -> feishin_core::styles::Target {
    let mut target = feishin_core::styles::Target::new(tag, name, &[]);
    target.classes = classes
        .iter()
        .map(|class| format!("fs-item-card-controls-module-{class}"))
        .collect();
    target
}

pub(crate) fn controls_stylesheet() -> &'static str {
    // Preserve the original two-class positioning specificity when custom CSS
    // uses a less-specific single-class selector.
    ".fs-item-card-controls-module-secondary-button.fs-item-card-controls-module-options { right:0; bottom:0; }
     .fs-item-card-controls-module-secondary-button.fs-item-card-controls-module-expand { left:0; bottom:0; }
     .fs-item-card-module-favorite-badge, .fs-item-card-module-rating-badge { opacity:1; }
     .fs-item-card-module-image-container:hover .fs-item-card-module-favorite-badge,
     .fs-item-card-module-image-container:hover .fs-item-card-module-rating-badge { opacity:0; }"
}

pub(crate) fn badge_style(
    class: &str,
    mut parents: Vec<feishin_core::styles::Target>,
    dim: f32,
    cx: &App,
) -> StyleRefinement {
    let Some(styles) = cx.try_global::<crate::styles::NativeStyles>() else {
        return StyleRefinement {
            opacity: Some(1. - dim),
            ..Default::default()
        };
    };
    let target = css_target("div", &[class]);
    let font = f32::from(cx.theme().font_size);
    let mut style = styles.style_in(target.clone(), parents.clone(), font);
    if let Some(image) = parents.last_mut() {
        image.states.retain(|state| state != "hover");
    }
    let shown = styles
        .style_in(target.clone(), parents.clone(), font)
        .opacity
        .unwrap_or(1.);
    if let Some(image) = parents.last_mut() {
        image.states.push("hover".into());
    }
    let hidden = styles.style_in(target, parents, font).opacity.unwrap_or(0.);
    style.opacity = Some(shown + (hidden - shown) * dim);
    style
}

use feishin_core::{
    model::{ItemKind, ServerType},
    queue::Enqueue,
};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, button::ButtonVariants, menu::DropdownMenu,
};
fn control_hitbox(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
}
fn control_layout(style: StyleRefinement) -> StyleRefinement {
    StyleRefinement {
        display: style.display,
        position: style.position,
        inset: style.inset,
        size: style.size,
        min_size: style.min_size,
        max_size: style.max_size,
        margin: style.margin,
        ..Default::default()
    }
}

fn control_paint(resolve: gpui_component::StateStyle) -> gpui_component::StateStyle {
    Rc::new(move |states, cx| {
        let mut style = resolve(states, cx);
        style.position = None;
        style.inset = Default::default();
        style.size = Default::default();
        style.min_size = Default::default();
        style.max_size = Default::default();
        style.margin = Default::default();
        style
    })
}

impl Feishin {
    pub(crate) fn card_controls(
        &self,
        item: &Item,
        index: usize,
        source: CardSource,
        artwork_size: Size<Pixels>,
        mut parents: Vec<feishin_core::styles::Target>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let font = cx.theme().font_size;
        let target = control_target("div", "", &["container"]);
        let mut controls = crate::styles::apply_in(
            div()
                .id(("card-controls", index))
                .debug_selector(move || format!("card-controls-{index}"))
                .relative()
                .size_full(),
            target.clone(),
            parents.clone(),
            cx,
        );
        parents.push(target);
        for (mode, center, fraction, name) in [
            (Enqueue::Now, 0.5, 0.25, "play"),
            (Enqueue::Next, 0.25, 0.15, "next"),
            (Enqueue::End, 0.75, 0.15, "last"),
        ] {
            let clicked = item.clone();
            let held = item.clone();
            let selector = format!("library-{name}-{index}");
            let diameter = artwork_size.width * fraction;
            let button_height = artwork_size.height * fraction;
            let mut target = control_target(
                "button",
                &selector,
                if matches!(mode, Enqueue::Now) {
                    &["play-button", "primary"]
                } else if matches!(mode, Enqueue::Next) {
                    &["play-button", "secondary", "left"]
                } else {
                    &["play-button", "secondary", "right"]
                },
            );
            let classes = target.classes.clone();
            target.classes.extend([
                "fs-play-button-module-play-button".into(),
                if matches!(mode, Enqueue::Now) {
                    "fs-play-button-module-fill"
                } else {
                    "fs-play-button-module-secondary"
                }
                .into(),
            ]);
            let resolve = crate::styles::state_style_path(target, parents.clone());
            let centered = Rc::new(move |states: &[&str], cx: &App| {
                let mut style = resolve(states, cx);
                style.position.get_or_insert(Position::Absolute);
                let font = cx.theme().font_size;
                let width = css_pixels(style.size.width, artwork_size.width, font, diameter);
                let height =
                    css_pixels(style.size.height, artwork_size.height, font, button_height);
                style.inset.left = Some(
                    (css_pixels(
                        style.inset.left,
                        artwork_size.width,
                        font,
                        artwork_size.width * center,
                    ) - width / 2.)
                        .into(),
                );
                style.inset.top = Some(
                    (css_pixels(
                        style.inset.top,
                        artwork_size.height,
                        font,
                        artwork_size.height * 0.5,
                    ) - height / 2.)
                        .into(),
                );
                style
            });
            let layout = control_layout(centered(&[], cx));
            controls = controls.child(
                control_hitbox(SharedString::from(format!("card-play-hit-{index}-{name}")))
                    .absolute()
                    .w(diameter)
                    .h(button_height)
                    .refine_style(&layout)
                    .child(
                        crate::styles::play_control_with_classes(
                            SharedString::from(selector),
                            mode,
                            self.state.settings.zoom,
                            parents.clone(),
                            &classes.iter().map(String::as_str).collect::<Vec<_>>(),
                            cx,
                        )
                        .state_style(control_paint(centered))
                        .with_size(diameter)
                        .size_full()
                        .rounded(diameter / 2.)
                        .aria_label(self.tr(match mode {
                            Enqueue::Now => "player.play",
                            Enqueue::Next => "player.addNext",
                            Enqueue::End => "player.addLast",
                        }))
                        .disabled(!self.library_fetches.is_empty())
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.play_card_item(source, &clicked, index, mode, false, cx);
                            cx.stop_propagation();
                        }))
                        .on_long_press(cx.listener(
                            move |view, _, _, cx| {
                                view.play_card_item(source, &held, index, mode, true, cx);
                                cx.stop_propagation();
                            },
                        )),
                    ),
            );
        }
        let secondary = |name: &str, class: &str, icon: &str| {
            let selector = format!("card-{name}-{index}");
            gpui_component::button::Button::new(SharedString::from(selector.clone()))
                .state_style(control_paint(crate::styles::state_style_path(
                    control_target("button", &selector, &["secondary-button", class]),
                    parents.clone(),
                )))
                .debug_selector(move || selector.clone())
                .ghost()
                .with_size(font * 2.625)
                .size_full()
                .p(font * 0.75)
                .rounded(font * 5. / 16.)
                .interaction_scale()
                .icon(
                    Icon::default()
                        .path(SharedString::from(icon.to_owned()))
                        .with_size(font * 1.125),
                )
        };
        let secondary_layout = |name: &str, class: &str| {
            let style = crate::styles::state_style_path(
                control_target(
                    "button",
                    &format!("card-{name}-{index}"),
                    &["secondary-button", class],
                ),
                parents.clone(),
            )(&[], cx);
            // The original plain buttons use `all: unset`, so explicit CSS
            // dimensions describe content; their padding sits outside it.
            let padding = |value: Option<DefiniteLength>| {
                value.map_or(font * 0.75, |value| {
                    value.to_pixels(artwork_size.width.into(), font)
                })
            };
            let extra_width = padding(style.padding.left) + padding(style.padding.right);
            let extra_height = padding(style.padding.top) + padding(style.padding.bottom);
            let mut layout = control_layout(style);
            if let Some(Length::Definite(value)) = layout.size.width {
                layout.size.width =
                    Some((value.to_pixels(artwork_size.width.into(), font) + extra_width).into());
            }
            if let Some(Length::Definite(value)) = layout.size.height {
                layout.size.height =
                    Some((value.to_pixels(artwork_size.height.into(), font) + extra_height).into());
            }
            layout
        };
        if self.state.settings.show_favorites
            && item.playlist_songs.is_empty()
            && matches!(
                item.kind,
                ItemKind::Song | ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
            )
        {
            let favorite = item.clone();
            controls = controls.child(
                control_hitbox(("card-favorite-hit", index))
                    .absolute()
                    .top_0()
                    .size(font * 2.625)
                    .map(|hit| {
                        if crate::appearance::rtl(cx) {
                            hit.right_0()
                        } else {
                            hit.left_0()
                        }
                    })
                    .refine_style(&secondary_layout("favorite", "favorite"))
                    .child(
                        secondary(
                            "favorite",
                            "favorite",
                            if item.favorite {
                                "native-icons/favorite-filled.svg"
                            } else {
                                "native-icons/favorite.svg"
                            },
                        )
                        .aria_label(self.tr("Favorite"))
                        .tooltip(self.tr("Favorite"))
                        .when(item.favorite, |button| {
                            button.text_color(cx.theme().primary)
                        })
                        .disabled(self.favorite_requests > 0)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.toggle_favorite(favorite.clone(), cx);
                            cx.stop_propagation();
                        })),
                    ),
            );
        }
        if self.state.settings.show_ratings
            && item.playlist_songs.is_empty()
            && self
                .sessions
                .get(&item.server_id)
                .is_some_and(|api| api.server.kind != ServerType::Jellyfin)
            && matches!(
                item.kind,
                ItemKind::Song | ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
            )
        {
            controls = controls.child(crate::styles::apply_in(
                control_hitbox(("card-rating-hit", index))
                    .debug_selector(move || format!("card-rating-control-{index}"))
                    .absolute()
                    .top_0()
                    .right_0()
                    .py(font * 0.75)
                    .child(
                        self.rating(format!("card-rating-{index}"), item, cx)
                            .size(font * 13. / 16.)
                            .horizontal_padding(font * 0.5)
                            .read_only(self.rating_requests > 0),
                    ),
                control_target("div", "", &["rating"]),
                parents.clone(),
                cx,
            ));
        }
        let action_items = if source == CardSource::Library && self.selected.contains(&index) {
            self.selected_library_items()
        } else {
            item.action_items()
        };
        let more_layout = secondary_layout("more", "options");
        controls = controls.child(
            control_hitbox(("card-more-hit", index))
                .absolute()
                .right_0()
                .bottom_0()
                .size(font * 2.625)
                .refine_style(&more_layout)
                .child(
                    secondary("more", "options", "native-icons/more.svg")
                        .w(css_pixels(
                            more_layout.size.width,
                            artwork_size.width,
                            font,
                            font * 2.625,
                        ))
                        .h(css_pixels(
                            more_layout.size.height,
                            artwork_size.height,
                            font,
                            font * 2.625,
                        ))
                        .aria_label(self.tr("More"))
                        .tooltip(self.tr("More"))
                        .dropdown_menu(self.item_menu_builder(item, action_items, false, cx)),
                ),
        );
        if item.kind == ItemKind::Album {
            let album = item.clone();
            controls = controls.child(
                control_hitbox(("card-expand-hit", index))
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .size(font * 2.625)
                    .refine_style(&secondary_layout("expand", "expand"))
                    .child(
                        secondary("expand", "expand", "native-icons/arrow-down.svg")
                            .debug_selector(move || format!("expand-album-{index}"))
                            .aria_label(self.tr("Expand"))
                            .tooltip(self.tr("Expand"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.toggle_expanded_album(album.clone(), cx);
                                cx.stop_propagation();
                            })),
                    ),
            );
        }
        controls.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{ControlsMotion, Duration, Instant};
    #[test]
    fn control_motion_matches_original_enter_exit_and_interrupted_reversals() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/item-card-motion-reference.json"))
                .unwrap();
        let epoch = Instant::now();
        for case in cases {
            let compact = case["type"] == "compact";
            let mut motion = ControlsMotion::new(compact);
            for frame in case["frames"].as_array().unwrap() {
                let now = epoch + Duration::from_millis(frame["time"].as_u64().unwrap());
                if let Some(show) = frame["action"].as_bool() {
                    motion.set_visible(show, now);
                }
                let actual = motion.frame(now);
                if frame["value"].is_null() {
                    assert!(actual.is_none(), "{case}: {frame}");
                } else {
                    let (opacity, position, _) = actual.unwrap();
                    let expected = &frame["value"];
                    assert!(
                        (opacity - expected["opacity"].as_f64().unwrap() as f32).abs() < 0.002,
                        "{frame}: opacity {opacity}"
                    );
                    let value = expected[if compact { "scale" } else { "y" }]
                        .as_f64()
                        .unwrap() as f32;
                    assert!(
                        (position - value).abs() < if compact { 0.002 } else { 0.03 },
                        "{frame}: position {position}"
                    );
                }
            }
        }
    }
}
