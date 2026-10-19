use std::ops::Range;

use crate::{ActiveTheme, AxisExt, FocusableExt, StyledExt, animation::CssTransition, h_flex};
use gpui::{
    Along, App, Axis, Background, Bounds, Context, Corners, Empty, Entity, EventEmitter, Hsla,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement as _, Pixels,
    Point, Render, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    canvas, div, prelude::FluentBuilder as _, px, relative,
};

/// Events emitted by the [`SliderState`].
pub enum SliderEvent {
    Change(SliderValue),
    ChangeEnd(SliderValue),
}

/// The value of the slider, can be a single value or a range of values.
///
/// - Can from a f32 value, which will be treated as a single value.
/// - Or from a (f32, f32) tuple, which will be treated as a range of values.
///
/// The default value is `SliderValue::Single(0.0)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderValue {
    Single(f32),
    Range(f32, f32),
}

impl std::fmt::Display for SliderValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SliderValue::Single(value) => write!(f, "{}", value),
            SliderValue::Range(start, end) => write!(f, "{}..{}", start, end),
        }
    }
}

impl From<f32> for SliderValue {
    fn from(value: f32) -> Self {
        SliderValue::Single(value)
    }
}

impl From<(f32, f32)> for SliderValue {
    fn from(value: (f32, f32)) -> Self {
        SliderValue::Range(value.0, value.1)
    }
}

impl From<Range<f32>> for SliderValue {
    fn from(value: Range<f32>) -> Self {
        SliderValue::Range(value.start, value.end)
    }
}

impl Default for SliderValue {
    fn default() -> Self {
        SliderValue::Single(0.)
    }
}

impl SliderValue {
    /// Clamp the value to the given range.
    pub fn clamp(self, min: f32, max: f32) -> Self {
        match self {
            SliderValue::Single(value) => SliderValue::Single(value.clamp(min, max)),
            SliderValue::Range(start, end) => {
                SliderValue::Range(start.clamp(min, max), end.clamp(min, max))
            }
        }
    }

    /// Check if the value is a single value.
    #[inline]
    pub fn is_single(&self) -> bool {
        matches!(self, SliderValue::Single(_))
    }

    /// Check if the value is a range of values.
    #[inline]
    pub fn is_range(&self) -> bool {
        matches!(self, SliderValue::Range(_, _))
    }

    /// Get the start value.
    pub fn start(&self) -> f32 {
        match self {
            SliderValue::Single(value) => *value,
            SliderValue::Range(start, _) => *start,
        }
    }

    /// Get the end value.
    pub fn end(&self) -> f32 {
        match self {
            SliderValue::Single(value) => *value,
            SliderValue::Range(_, end) => *end,
        }
    }

    fn set_start(&mut self, value: f32) {
        if let SliderValue::Range(_, end) = self {
            *self = SliderValue::Range(value.min(*end), *end);
        } else {
            *self = SliderValue::Single(value);
        }
    }

    fn set_end(&mut self, value: f32) {
        if let SliderValue::Range(start, _) = self {
            *self = SliderValue::Range(*start, value.max(*start));
        } else {
            *self = SliderValue::Single(value);
        }
    }
}

/// The scale mode of the slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderScale {
    /// Linear scale where values change uniformly across the slider range.
    /// This is the default mode.
    #[default]
    Linear,
    /// Logarithmic scale where the distance between values increases exponentially.
    ///
    /// This is useful for parameters that have a large range of values where smaller
    /// changes are more significant at lower values. Common examples include:
    ///
    /// - Volume controls (human hearing perception is logarithmic)
    /// - Frequency controls (musical notes follow a logarithmic scale)
    /// - Zoom levels
    /// - Any parameter where you want finer control at lower values
    ///
    /// # For example
    ///
    /// ```
    /// use gpui_component::slider::{SliderState, SliderScale};
    ///
    /// let slider = SliderState::new()
    ///     .min(1.0)    // Must be > 0 for logarithmic scale
    ///     .max(1000.0)
    ///     .scale(SliderScale::Logarithmic);
    /// ```
    ///
    /// - Moving the slider 1/3 of the way will yield ~10
    /// - Moving it 2/3 of the way will yield ~100
    /// - The full range covers 3 orders of magnitude evenly
    Logarithmic,
}

impl SliderScale {
    #[inline]
    pub fn is_linear(&self) -> bool {
        matches!(self, SliderScale::Linear)
    }

    #[inline]
    pub fn is_logarithmic(&self) -> bool {
        matches!(self, SliderScale::Logarithmic)
    }
}

/// State of the [`Slider`].
pub struct SliderState {
    min: f32,
    max: f32,
    step: f32,
    value: SliderValue,
    /// When is single value mode, only `end` is used, the start is always 0.0.
    percentage: Range<f32>,
    /// The bounds of the slider after rendered.
    bounds: Bounds<Pixels>,
    scale: SliderScale,
    pointer_changed: bool,
    dragging: Option<bool>,
    focus_handle: Option<gpui::FocusHandle>,
}

impl SliderState {
    /// Create a new [`SliderState`].
    pub fn new() -> Self {
        Self {
            min: 0.0,
            max: 100.0,
            step: 1.0,
            value: SliderValue::default(),
            percentage: (0.0..0.0),
            bounds: Bounds::default(),
            scale: SliderScale::default(),
            pointer_changed: false,
            dragging: None,
            focus_handle: None,
        }
    }

    /// Set the minimum value of the slider, default: 0.0
    pub fn min(mut self, min: f32) -> Self {
        if self.scale.is_logarithmic() {
            assert!(
                min > 0.0,
                "`min` must be greater than 0 for SliderScale::Logarithmic"
            );
            assert!(
                min < self.max,
                "`min` must be less than `max` for Logarithmic scale"
            );
        }
        self.min = min;
        self.update_thumb_pos();
        self
    }

    /// Set the maximum value of the slider, default: 100.0
    pub fn max(mut self, max: f32) -> Self {
        if self.scale.is_logarithmic() {
            assert!(
                max > self.min,
                "`max` must be greater than `min` for Logarithmic scale"
            );
        }
        self.max = max;
        self.update_thumb_pos();
        self
    }

    /// Set the step value of the slider, default: 1.0
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Set the scale of the slider, default: [`SliderScale::Linear`].
    pub fn scale(mut self, scale: SliderScale) -> Self {
        if scale.is_logarithmic() {
            assert!(
                self.min > 0.0,
                "`min` must be greater than 0 for Logarithmic scale"
            );
            assert!(
                self.max > self.min,
                "`max` must be greater than `min` for Logarithmic scale"
            );
        }
        self.scale = scale;
        self.update_thumb_pos();
        self
    }

    /// Set the default value of the slider, default: 0.0
    pub fn default_value(mut self, value: impl Into<SliderValue>) -> Self {
        self.value = value.into();
        self.update_thumb_pos();
        self
    }

    /// Set the value of the slider.
    pub fn set_value(
        &mut self,
        value: impl Into<SliderValue>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.value = value.into();
        self.update_thumb_pos();
        cx.notify();
    }

    /// Update a changing domain without replacing the subscribed state.
    pub fn set_range(&mut self, min: f32, max: f32, cx: &mut Context<Self>) {
        assert!(min.is_finite() && max.is_finite() && min <= max);
        assert!(!self.scale.is_logarithmic() || (min > 0. && max > min));
        if self.min != min || self.max != max {
            self.min = min;
            self.max = max;
            self.update_thumb_pos();
            cx.notify();
        }
    }

    /// Cancel an edit when the item represented by this control changes.
    pub fn cancel_pointer_change(&mut self) {
        self.dragging = None;
        self.pointer_changed = false;
    }

    /// Get the value of the slider.
    pub fn value(&self) -> SliderValue {
        self.value
    }

    /// Converts a value between 0.0 and 1.0 to a value between the minimum and maximum value,
    /// depending on the chosen scale.
    fn percentage_to_value(&self, percentage: f32) -> f64 {
        let min = Self::decimal(self.min);
        let max = Self::decimal(self.max);
        let percentage = f64::from(percentage);
        match self.scale {
            SliderScale::Linear => min + (max - min) * percentage,
            SliderScale::Logarithmic => {
                // when percentage is 0, this simplifies to (max/min)^0 * min = 1 * min = min
                // when percentage is 1, this simplifies to (max/min)^1 * min = (max*min)/min = max
                // we clamp just to make sure we don't have issue with floating point precision
                let base = max / min;
                (base.powf(percentage) * min).clamp(min, max)
            }
        }
    }

    /// Converts a value between the minimum and maximum value to a value between 0.0 and 1.0,
    /// depending on the chosen scale.
    fn value_to_percentage(&self, value: f32) -> f32 {
        match self.scale {
            SliderScale::Linear => {
                let range = self.max - self.min;
                if range <= 0.0 {
                    0.0
                } else {
                    (value - self.min) / range
                }
            }
            SliderScale::Logarithmic => {
                let base = self.max / self.min;
                (value / self.min).log(base).clamp(0.0, 1.0)
            }
        }
    }

    fn update_thumb_pos(&mut self) {
        match self.value {
            SliderValue::Single(value) => {
                let percentage = self.value_to_percentage(value.clamp(self.min, self.max));
                self.percentage = 0.0..percentage;
            }
            SliderValue::Range(start, end) => {
                let clamped_start = start.clamp(self.min, self.max);
                let clamped_end = end.clamp(self.min, self.max);
                self.percentage =
                    self.value_to_percentage(clamped_start)..self.value_to_percentage(clamped_end);
            }
        }
    }

    fn update_accessible_value(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        if !value.is_finite() {
            return;
        }
        let step = Self::decimal(self.step);
        let min = Self::decimal(self.min);
        let value = if step > 0. {
            ((value - min) / step).round() * step + min
        } else {
            value
        };
        let value = value.clamp(self.min as f64, self.max as f64) as f32;
        let mut next = self.value;
        // Range controls adjust the upper endpoint, preserving their lower bound.
        match next {
            SliderValue::Single(_) => next = SliderValue::Single(value),
            SliderValue::Range(start, _) => next.set_end(value.max(start)),
        }
        if next != self.value {
            self.set_value(next, window, cx);
            cx.emit(SliderEvent::Change(next));
            cx.emit(SliderEvent::ChangeEnd(next));
        }
    }

    fn decimal(value: f32) -> f64 {
        // Preserve configured decimal steps when rounding at half-step boundaries.
        value.to_string().parse().unwrap_or(f64::from(value))
    }

    /// Update value by mouse position
    fn update_value_by_position(
        &mut self,
        axis: Axis,
        rtl: bool,
        position: Point<Pixels>,
        is_start: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = self.bounds;
        let step = Self::decimal(self.step);

        let inner_pos = if axis.is_horizontal() && rtl {
            bounds.right() - position.x
        } else if axis.is_horizontal() {
            position.x - bounds.left()
        } else {
            bounds.bottom() - position.y
        };
        let total_size = bounds.size.along(axis);
        if total_size <= px(0.) {
            return;
        }
        let percentage = inner_pos.clamp(px(0.), total_size) / total_size;

        let percentage = if is_start {
            percentage.clamp(0.0, self.percentage.end)
        } else {
            percentage.clamp(self.percentage.start, 1.0)
        };

        let value = self.percentage_to_value(percentage);
        let min = Self::decimal(self.min);
        let value = if step > 0. {
            ((value - min) / step).round() * step + min
        } else {
            value
        }
        .clamp(Self::decimal(self.min), Self::decimal(self.max)) as f32;

        if is_start {
            self.percentage.start = percentage;
            self.value.set_start(value);
        } else {
            self.percentage.end = percentage;
            self.value.set_end(value);
        }
        self.update_thumb_pos();
        self.pointer_changed = true;
        cx.emit(SliderEvent::Change(self.value));
        cx.notify();
    }

    fn end_pointer_change(&mut self, cx: &mut Context<Self>) {
        self.dragging = None;
        if std::mem::take(&mut self.pointer_changed) {
            cx.emit(SliderEvent::ChangeEnd(self.value));
        }
    }

    fn keyboard_value(
        &mut self,
        key: &str,
        rtl: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let value = match key {
            "home" => self.min,
            "end" => self.max,
            "up" => self.value.end() + self.step,
            "down" => self.value.end() - self.step,
            "right" => self.value.end() + if rtl { -self.step } else { self.step },
            "left" => self.value.end() + if rtl { self.step } else { -self.step },
            _ => return false,
        }
        .clamp(self.min, self.max);
        let precision = self
            .step
            .to_string()
            .split_once('.')
            .map_or(0, |(_, digits)| digits.len())
            .min(10) as i32;
        let factor = 10f64.powi(precision);
        let value = if matches!(key, "home" | "end") {
            value
        } else {
            ((f64::from(value) * factor).round() / factor) as f32
        };
        let next = match self.value {
            SliderValue::Single(_) => SliderValue::Single(value),
            SliderValue::Range(start, _) => SliderValue::Range(start, value.max(start)),
        };
        self.set_value(next, window, cx);
        cx.emit(SliderEvent::Change(next));
        cx.emit(SliderEvent::ChangeEnd(next));
        true
    }
}

impl EventEmitter<SliderEvent> for SliderState {}
impl Render for SliderState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// A Slider element.
#[derive(IntoElement)]
pub struct Slider {
    state: Entity<SliderState>,
    axis: Axis,
    style: StyleRefinement,
    disabled: bool,
    rtl: bool,
    aria_label: Option<gpui::SharedString>,
    value_label: Option<gpui::SharedString>,
    playerbar_zoom: Option<f32>,
    playerbar_prefix: &'static str,
    settings: bool,
    track_background_style: StyleRefinement,
}

impl Slider {
    /// Create a new [`Slider`] element bind to the [`SliderState`].
    pub fn new(state: &Entity<SliderState>) -> Self {
        Self {
            axis: Axis::Horizontal,
            state: state.clone(),
            style: StyleRefinement::default(),
            disabled: false,
            rtl: false,
            aria_label: None,
            value_label: None,
            playerbar_zoom: None,
            playerbar_prefix: "slider",
            settings: false,
            track_background_style: StyleRefinement::default(),
        }
    }

    /// Set a screen-reader label for the slider.
    pub fn aria_label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Show the formatted value above the thumb on hover or while editing.
    pub fn value_label(mut self, label: impl Into<gpui::SharedString>) -> Self {
        self.value_label = Some(label.into());
        self
    }

    /// Apply Feishin's compact player-bar track, thumb and formatted-value style.
    pub fn playerbar(mut self, zoom: f32, prefix: &'static str) -> Self {
        self.playerbar_zoom = Some(zoom);
        self.playerbar_prefix = prefix;
        self
    }

    /// Apply Feishin's wrapped settings slider dimensions and persistent thumb.
    pub fn settings(mut self, zoom: f32, prefix: &'static str) -> Self {
        self.playerbar_zoom = Some(zoom);
        self.playerbar_prefix = prefix;
        self.settings = true;
        self
    }

    /// Style the unfilled track independently of the interactive control.
    pub fn track_background_style(mut self, style: StyleRefinement) -> Self {
        self.track_background_style = style;
        self
    }

    /// Mirror horizontal movement and artwork for right-to-left layouts.
    pub fn rtl(mut self, rtl: bool) -> Self {
        self.rtl = rtl;
        self
    }

    /// As a horizontal slider.
    pub fn horizontal(mut self) -> Self {
        self.axis = Axis::Horizontal;
        self
    }

    /// As a vertical slider.
    pub fn vertical(mut self) -> Self {
        self.axis = Axis::Vertical;
        self
    }

    /// Set the disabled state of the slider, default: false
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[allow(clippy::too_many_arguments)]
    fn render_thumb(
        &self,
        start_pos: f32,
        is_start: bool,
        bar_color: Background,
        thumb_color: Hsla,
        radius: Corners<Pixels>,
        emphasis: f32,
        show_label: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> impl gpui::IntoElement {
        let axis = self.axis;
        let rtl = self.rtl;
        let start_pos = if axis.is_horizontal() && rtl {
            1. - start_pos
        } else {
            start_pos
        };
        let id = ("slider-thumb", is_start as u32);
        let editing = self.state.read(cx).dragging.is_some();
        let zoom = self.playerbar_zoom.unwrap_or(1.);
        let compact = self.playerbar_zoom.is_some();
        let prefix = self.playerbar_prefix;
        let thumb_size = window.rem_size();

        if self.disabled {
            return div().id(id);
        }

        div()
            .id(id)
            .group("slider-thumb")
            .absolute()
            .when(axis.is_horizontal(), |this| {
                this.top(px(-5.)).left(relative(start_pos)).ml(-px(8.))
            })
            .when(axis.is_vertical(), |this| {
                this.bottom(relative(start_pos)).left(px(-5.)).mb(-px(8.))
            })
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .corner_radii(radius)
            .bg(bar_color.opacity(0.5))
            .when(cx.theme().shadow, |this| this.shadow_md())
            .size_4()
            .p(px(1.))
            .when(compact, |this| {
                this.debug_selector(move || format!("{prefix}-thumb"))
                    .size(thumb_size)
                    .top(-thumb_size / 4.)
                    .ml(-thumb_size / 2.)
                    .when(rtl, |this| this.ml(-thumb_size * 0.75))
                    .p_0()
                    .border(px(zoom))
                    .border_color(cx.theme().primary)
                    .bg(if cx.theme().mode.is_dark() {
                        cx.theme().primary
                    } else {
                        gpui::white()
                    })
                    .shadow_none()
                    .when(editing, |this| {
                        this.paint_scale(1.05).shadow(
                            [(1., 3., 0., 0.1), (1., 2., 0., 0.06)]
                                .map(|(y, blur, spread, alpha)| gpui::BoxShadow {
                                    color: gpui::black().opacity(alpha),
                                    offset: gpui::point(px(0.), px(y * zoom)),
                                    blur_radius: px(blur * zoom),
                                    spread_radius: px(spread * zoom),
                                })
                                .to_vec(),
                        )
                    })
                    .opacity(emphasis)
                    .when(self.settings, |this| {
                        this.border(thumb_size * 0.25)
                            .border_color(if cx.theme().mode.is_dark() {
                                gpui::white()
                            } else {
                                cx.theme().primary
                            })
                            .bg(cx.theme().foreground)
                            .when(rtl, |this| this.ml(-thumb_size / 2.))
                    })
            })
            .when_some(
                self.value_label.clone().filter(|_| !compact || show_label),
                |this, label| {
                    this.child(
                        div()
                            .id("slider-value-label")
                            .absolute()
                            .bottom(px(24.))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(cx.theme().foreground)
                            .text_color(cx.theme().background)
                            .text_xs()
                            .whitespace_nowrap()
                            .when(compact, |this| {
                                this.debug_selector(move || format!("{prefix}-label"))
                                    .top(-thumb_size * 2.25)
                                    .bottom_auto()
                                    .max_w(px(200. * zoom))
                                    .py(gpui::rems(0.5))
                                    .px(gpui::rems(0.75))
                                    .text_size(gpui::rems(0.875))
                                    .line_height(gpui::rems(1.125))
                                    .font_weight(gpui::FontWeight(550.))
                                    .rounded(gpui::rems(0.1875))
                                    .bg(cx.theme().secondary)
                                    .text_color(cx.theme().secondary_foreground)
                                    .shadow(vec![gpui::BoxShadow {
                                        color: gpui::black().opacity(0.2),
                                        offset: gpui::point(px(4. * zoom), px(4. * zoom)),
                                        blur_radius: px(10. * zoom),
                                        spread_radius: px(0.),
                                    }])
                            })
                            .when(!editing && !compact, |this| {
                                this.invisible()
                                    .group_hover("slider-thumb", |this| this.visible())
                            })
                            .child(label),
                    )
                },
            )
            .when(!compact, |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .size_full()
                        .corner_radii(radius)
                        .bg(thumb_color),
                )
            })
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(
                    &self.state,
                    move |state, event: &MouseDownEvent, window, cx| {
                        if let Some(focus) = &state.focus_handle {
                            window.focus(focus);
                        }
                        state.dragging = Some(is_start);
                        state.update_value_by_position(
                            axis,
                            rtl,
                            event.position,
                            is_start,
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                    },
                ),
            )
    }
}

impl Styled for Slider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let axis = self.axis;
        let rtl = self.rtl;
        let focus = self.state.update(cx, |state, cx| {
            if self.disabled {
                state.dragging = None;
                state.pointer_changed = false;
            }
            state
                .focus_handle
                .get_or_insert_with(|| cx.focus_handle())
                .clone()
        });
        let focused = focus.is_focused(window);
        let compact = self.playerbar_zoom.is_some();
        let prefix = self.playerbar_prefix;
        let motion = compact.then(|| {
            window.use_keyed_state(("slider-emphasis", self.state.entity_id()), cx, |_, _| {
                (
                    false,
                    CssTransition::new(
                        0.,
                        std::time::Duration::from_millis(200),
                        [0.42, 0., 0.58, 1.],
                    ),
                )
            })
        });
        let (mut emphasis, hovered) = motion.as_ref().map_or((1., false), |motion| {
            motion.update(cx, |(hovered, transition), cx| {
                let now = cx.background_executor().now();
                transition.retarget(f32::from(*hovered || focused), now);
                let (value, animating) = transition.value(now);
                if animating {
                    window.request_animation_frame();
                }
                (value, *hovered)
            })
        });
        if self.settings {
            emphasis = 1.;
        }
        let state = self.state.read(cx);
        let show_label = hovered || focused || state.dragging.is_some();
        let is_range = state.value().is_range();
        let bar_start = state.percentage.start;
        let bar_end = state.percentage.end;
        let rem_size = window.rem_size();

        let mut bar_color = self
            .style
            .background
            .clone()
            .and_then(|bg| bg.color())
            .unwrap_or(cx.theme().slider_bar.into());
        if compact {
            bar_color = cx
                .theme()
                .foreground
                .blend(cx.theme().primary.opacity(emphasis))
                .into();
        }
        let thumb_color = self
            .style
            .text
            .clone()
            .and_then(|text| text.color)
            .unwrap_or_else(|| cx.theme().slider_thumb);
        let corner_radii = self.style.corner_radii.clone();
        let default_radius = px(999.);
        let radius = Corners {
            top_left: corner_radii
                .top_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            top_right: corner_radii
                .top_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            bottom_left: corner_radii
                .bottom_left
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
            bottom_right: corner_radii
                .bottom_right
                .map(|v| v.to_pixels(rem_size))
                .unwrap_or(default_radius),
        };

        div()
            .id(("slider", self.state.entity_id()))
            .key_context("FormControl")
            .role(gpui::Role::Slider)
            .aria_disabled(self.disabled)
            .when_some(self.aria_label.clone(), |this, label| {
                this.aria_label(label)
            })
            .aria_numeric_value(state.value().end() as f64)
            .aria_min_numeric_value(state.min as f64)
            .aria_max_numeric_value(state.max as f64)
            .aria_orientation(if axis.is_vertical() {
                gpui::Orientation::Vertical
            } else {
                gpui::Orientation::Horizontal
            })
            .when(!self.disabled, |this| {
                this.track_focus(&focus.clone().tab_stop(true))
                    .on_key_down(window.listener_for(
                        &self.state,
                        move |state, event: &gpui::KeyDownEvent, window, cx| {
                            if state.keyboard_value(&event.keystroke.key, rtl, window, cx) {
                                window.prevent_default();
                                cx.stop_propagation();
                            }
                        },
                    ))
                    .on_mouse_up(
                        MouseButton::Left,
                        window.listener_for(&self.state, |state, _, _, cx| {
                            state.end_pointer_change(cx)
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        window.listener_for(&self.state, |state, _, _, cx| {
                            state.end_pointer_change(cx)
                        }),
                    )
                    .on_a11y_action(
                        gpui::AccessibleAction::SetValue,
                        window.listener_for(&self.state, |state, data, window, cx| {
                            if let Some(gpui::accesskit::ActionData::NumericValue(value)) = data {
                                state.update_accessible_value(*value, window, cx);
                            }
                        }),
                    )
                    .on_a11y_action(
                        gpui::AccessibleAction::Increment,
                        window.listener_for(&self.state, |state, _, window, cx| {
                            state.update_accessible_value(
                                (state.value().end() + state.step) as f64,
                                window,
                                cx,
                            );
                        }),
                    )
                    .on_a11y_action(
                        gpui::AccessibleAction::Decrement,
                        window.listener_for(&self.state, |state, _, window, cx| {
                            state.update_accessible_value(
                                (state.value().end() - state.step) as f64,
                                window,
                                cx,
                            );
                        }),
                    )
            })
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .when(axis.is_vertical(), |this| this.h(px(120.)))
            .when(axis.is_horizontal(), |this| this.w_full())
            .refine_style(&self.style)
            .when(compact, |this| {
                this.debug_selector(move || format!("{prefix}-slider"))
                    .h(gpui::rems(0.75))
                    .px(gpui::rems(0.375))
                    .when(self.settings, |this| {
                        this.h(gpui::rems(1.)).px(gpui::rems(0.5))
                    })
            })
            .when_some(motion, |this, motion| {
                this.on_hover(move |hovered, _, cx| {
                    motion.update(cx, |(previous, _), cx| {
                        *previous = *hovered;
                        cx.notify();
                    });
                })
            })
            .focus_ring(focused && !self.disabled, px(2.), window, cx)
            .bg(cx.theme().transparent)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .id("slider-bar-container")
                    .when(!self.disabled, |this| {
                        this.on_mouse_down(
                            MouseButton::Left,
                            window.listener_for(
                                &self.state,
                                move |state, e: &MouseDownEvent, window, cx| {
                                    if let Some(focus) = &state.focus_handle {
                                        window.focus(focus);
                                    }
                                    let mut is_start = false;
                                    if is_range {
                                        let inner_pos = if axis.is_horizontal() && rtl {
                                            state.bounds.right() - e.position.x
                                        } else if axis.is_horizontal() {
                                            e.position.x - state.bounds.left()
                                        } else {
                                            state.bounds.bottom() - e.position.y
                                        };
                                        let center = state.bounds.size.along(axis)
                                            * ((bar_end - bar_start) / 2.0 + bar_start);
                                        is_start = inner_pos < center;
                                    }

                                    state.dragging = Some(is_start);
                                    state.update_value_by_position(
                                        axis, rtl, e.position, is_start, window, cx,
                                    )
                                },
                            ),
                        )
                    })
                    .when(axis.is_horizontal(), |this| {
                        this.items_center()
                            .h_6()
                            .w_full()
                            .when(compact, |this| this.h(gpui::rems(0.75)))
                            .when(self.settings, |this| this.h(gpui::rems(1.)))
                    })
                    .when(axis.is_vertical(), |this| {
                        this.justify_center().w_6().h_full()
                    })
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("slider-bar")
                            .relative()
                            .when(axis.is_horizontal(), |this| this.w_full().h_1p5())
                            .when(axis.is_vertical(), |this| this.h_full().w_1p5())
                            .bg(bar_color.opacity(0.2))
                            .active(|this| this.bg(bar_color.opacity(0.4)))
                            .corner_radii(radius)
                            .when(!compact, |this| {
                                this.refine_style(&self.track_background_style)
                            })
                            .when(compact, |this| {
                                this.debug_selector(move || format!("{prefix}-track"))
                                    .h(gpui::rems(0.5))
                                    .bg(gpui::transparent_black())
                                    .child(
                                        div()
                                            .debug_selector(move || {
                                                format!("{prefix}-track-background")
                                            })
                                            .absolute()
                                            .top_0()
                                            .bottom_0()
                                            .left(gpui::rems(-0.375))
                                            .right(gpui::rems(-0.375))
                                            .when(self.settings, |this| {
                                                this.left(gpui::rems(-0.5)).right(gpui::rems(-0.5))
                                            })
                                            .rounded_full()
                                            .bg(gpui::rgb(if cx.theme().mode.is_dark() {
                                                0x424242
                                            } else {
                                                0xe9ecef
                                            }))
                                            .refine_style(&self.track_background_style),
                                    )
                            })
                            .child(
                                div()
                                    .id("slider-fill")
                                    .absolute()
                                    .when(axis.is_horizontal(), |this| {
                                        this.h_full()
                                            .left(relative(if rtl {
                                                1. - bar_end
                                            } else {
                                                bar_start
                                            }))
                                            .right(relative(if rtl {
                                                bar_start
                                            } else {
                                                1. - bar_end
                                            }))
                                    })
                                    .when(axis.is_vertical(), |this| {
                                        this.w_full()
                                            .bottom(relative(bar_start))
                                            .top(relative(1. - bar_end))
                                    })
                                    .when(compact, |this| {
                                        this.debug_selector(move || format!("{prefix}-fill"))
                                            .ml(gpui::rems(-0.375))
                                            .mr(gpui::rems(-0.375))
                                            .when(self.settings, |this| {
                                                this.ml(gpui::rems(-0.5)).mr(gpui::rems(-0.5))
                                            })
                                    })
                                    .bg(bar_color)
                                    .rounded_full(),
                            )
                            .when(is_range, |this| {
                                this.child(self.render_thumb(
                                    bar_start,
                                    true,
                                    bar_color,
                                    thumb_color,
                                    radius,
                                    emphasis,
                                    show_label,
                                    window,
                                    cx,
                                ))
                            })
                            .child(self.render_thumb(
                                bar_end,
                                false,
                                bar_color,
                                thumb_color,
                                radius,
                                emphasis,
                                show_label,
                                window,
                                cx,
                            ))
                            .child({
                                let state = self.state.clone();
                                let pointer = self.state.clone();
                                let disabled = self.disabled;
                                canvas(
                                    move |bounds, _, cx| state.update(cx, |r, _| r.bounds = bounds),
                                    move |_, _, window, _| {
                                        if disabled {
                                            return;
                                        }
                                        let release = pointer.clone();
                                        window.on_mouse_event(
                                            move |event: &gpui::MouseMoveEvent,
                                                  phase,
                                                  window,
                                                  cx| {
                                                if phase == gpui::DispatchPhase::Bubble
                                                    && event.pressed_button
                                                        == Some(MouseButton::Left)
                                                    && let Some(is_start) =
                                                        pointer.read(cx).dragging
                                                {
                                                    pointer.update(cx, |state, cx| {
                                                        state.update_value_by_position(
                                                            axis,
                                                            rtl,
                                                            event.position,
                                                            is_start,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }
                                            },
                                        );
                                        window.on_mouse_event(
                                            move |event: &gpui::MouseUpEvent, phase, _, cx| {
                                                if phase == gpui::DispatchPhase::Capture
                                                    && event.button == MouseButton::Left
                                                    && release.read(cx).dragging.is_some()
                                                {
                                                    release.update(cx, |state, cx| {
                                                        state.end_pointer_change(cx)
                                                    });
                                                }
                                            },
                                        );
                                    },
                                )
                                .absolute()
                                .size_full()
                            }),
                    ),
            )
    }
}
