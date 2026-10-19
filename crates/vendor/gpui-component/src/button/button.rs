use crate::animation::{CssColorTransition, CssTransition};
use std::rc::Rc;

use crate::{
    ActiveTheme, Colorize as _, Disableable, FocusableExt as _, Icon, IconName, Selectable,
    Sizable, Size, StyleSized, StyledExt, h_flex, spinner::Spinner, tooltip::Tooltip,
};
use gpui::{
    Action, AnyElement, App, ClickEvent, Corners, Div, Edges, ElementId, FocusHandle, Hsla,
    InteractiveElement, Interactivity, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Refineable, RenderOnce, SharedString, Stateful, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Task, Window, div, prelude::FluentBuilder as _, px, relative,
};

#[derive(Default)]
struct LongPressState {
    pressed: bool,
    canceled: bool,
    fired: bool,
    timer: Option<Task<()>>,
    started: Option<std::time::Instant>,
}

impl LongPressState {
    fn cancel(&mut self) {
        self.timer = None;
        self.pressed = false;
        self.canceled = true;
        self.started = None;
    }
}

struct InteractionScaleState {
    hovered: bool,
    pressed: bool,
    disabled: bool,
    hover_scale: f32,
    pressed_scale: f32,
    scale: CssTransition,
    opacity: CssTransition,
    background: CssTransition,
}
impl InteractionScaleState {
    fn new(disabled: bool, config: Option<(f32, f32, std::time::Duration, [f32; 4])>) -> Self {
        let (hover_scale, pressed_scale, duration, curve) = config.unwrap_or((
            1.1,
            0.9,
            std::time::Duration::from_millis(200),
            [0.42, 0., 0.58, 1.],
        ));
        Self {
            hovered: false,
            pressed: false,
            disabled,
            hover_scale,
            pressed_scale,
            scale: CssTransition::new(1., duration, curve),
            opacity: CssTransition::new(if disabled { 0.5 } else { 1. }, duration, curve),
            background: CssTransition::new(
                0.,
                std::time::Duration::from_millis(200),
                [0.42, 0., 0.58, 1.],
            ),
        }
    }
    fn retarget(&mut self, now: std::time::Instant) {
        self.scale.retarget(
            if self.pressed && !self.disabled {
                self.pressed_scale
            } else if self.hovered {
                self.hover_scale
            } else {
                1.
            },
            now,
        );
        self.opacity
            .retarget(if self.disabled { 0.5 } else { 1. }, now);
    }
}

struct WrappedPaint {
    background: CssColorTransition,
    border: CssColorTransition,
    foreground: CssColorTransition,
    opacity: CssTransition,
    colors: [Hsla; 3],
    legacy: [bool; 3],
    spaces: [gpui::ColorSpace; 3],
}

/// Cascaded paint transitions for an application-wrapped button.
#[derive(Clone, Copy)]
pub struct ButtonPaintMotion {
    /// Background, border, foreground and opacity, in that order.
    pub timing: [crate::animation::CssTiming; 4],
    pub legacy_colors: [bool; 3],
}
pub type ButtonPaintMotionStyle = Rc<dyn Fn(&[&str], &App) -> Option<ButtonPaintMotion>>;

struct WrappedLoading {
    contents: CssTransition,
    loader: CssTransition,
}
impl WrappedLoading {
    fn new(loading: bool) -> Self {
        let ease = [0.25, 0.1, 0.25, 1.];
        Self {
            contents: CssTransition::new(
                f32::from(!loading),
                std::time::Duration::from_millis(100),
                ease,
            ),
            loader: CssTransition::new(
                f32::from(loading),
                std::time::Duration::from_millis(150),
                ease,
            ),
        }
    }
    fn update(&mut self, loading: bool, now: std::time::Instant) -> (f32, f32, bool) {
        self.contents.retarget(f32::from(!loading), now);
        self.loader.retarget(f32::from(loading), now);
        let (contents, a) = self.contents.value(now);
        let (loader, b) = self.loader.value(now);
        (contents, loader, a || b)
    }
}
impl WrappedPaint {
    fn new(
        background: Hsla,
        border: Hsla,
        foreground: Hsla,
        opacity: f32,
        variant: ButtonVariant,
        outline: bool,
        motion: Option<ButtonPaintMotion>,
    ) -> Self {
        use std::time::Duration;
        let transparent = matches!(variant, ButtonVariant::Text | ButtonVariant::Link) && !outline;
        let filled = variant == ButtonVariant::Primary && !outline;
        let normal = Duration::from_millis(200);
        let ease = [0.42, 0., 0.58, 1.];
        let legacy = motion.map_or([outline, false, false], |motion| motion.legacy_colors);
        let spaces = legacy.map(|legacy| {
            if legacy {
                gpui::ColorSpace::Srgb
            } else {
                gpui::ColorSpace::Oklab
            }
        });
        Self {
            colors: [background, border, foreground],
            legacy,
            spaces,
            background: CssColorTransition::new(
                background,
                spaces[0],
                if transparent { Duration::ZERO } else { normal },
                ease,
            ),
            border: CssColorTransition::new(
                border,
                spaces[1],
                if transparent || filled {
                    Duration::ZERO
                } else {
                    normal
                },
                ease,
            ),
            foreground: CssColorTransition::new(
                foreground,
                spaces[2],
                if transparent { normal } else { Duration::ZERO },
                ease,
            ),
            opacity: CssTransition::new(
                opacity,
                if transparent || filled {
                    Duration::ZERO
                } else {
                    normal
                },
                [0.25, 0.1, 0.25, 1.],
            ),
        }
    }
    fn update(
        &mut self,
        background: Hsla,
        border: Hsla,
        foreground: Hsla,
        opacity: f32,
        motion: Option<ButtonPaintMotion>,
        now: std::time::Instant,
    ) -> (StyleRefinement, bool) {
        if let Some(motion) = motion {
            let colors = [background, border, foreground];
            for (index, color) in colors.iter().enumerate() {
                if *color != self.colors[index] {
                    self.spaces[index] = if self.legacy[index] && motion.legacy_colors[index] {
                        gpui::ColorSpace::Srgb
                    } else {
                        gpui::ColorSpace::Oklab
                    };
                }
            }
            self.colors = colors;
            self.legacy = motion.legacy_colors;
            self.background.retarget_with_options(
                background,
                self.spaces[0],
                motion.timing[0],
                now,
            );
            self.border
                .retarget_with_options(border, self.spaces[1], motion.timing[1], now);
            self.foreground.retarget_with_options(
                foreground,
                self.spaces[2],
                motion.timing[2],
                now,
            );
            self.opacity
                .retarget_with_options(opacity, motion.timing[3], now);
        } else {
            self.background.retarget(background, now);
            self.border.retarget(border, now);
            self.foreground.retarget(foreground, now);
            self.opacity.retarget(opacity, now);
        }
        let (background, a) = self.background.value(now);
        let (border, b) = self.border.value(now);
        let (foreground, c) = self.foreground.value(now);
        let (opacity, d) = self.opacity.value(now);
        let mut style = StyleRefinement::default();
        style.background = Some(background.into());
        style.border_color = Some(border);
        style.text.get_or_insert_with(Default::default).color = Some(foreground);
        style.opacity = Some(opacity.clamp(0., 1.));
        (style, a || b || c || d)
    }
}

#[derive(Default, Clone, Copy)]
pub enum ButtonRounded {
    None,
    Small,
    #[default]
    Medium,
    Large,
    Size(Pixels),
}

impl From<Pixels> for ButtonRounded {
    fn from(px: Pixels) -> Self {
        ButtonRounded::Size(px)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ButtonCustomVariant {
    color: Hsla,
    foreground: Hsla,
    border: Hsla,
    shadow: bool,
    hover: Hsla,
    active: Hsla,
}

pub trait ButtonVariants: Sized {
    fn with_variant(self, variant: ButtonVariant) -> Self;

    /// With the primary style for the Button.
    fn primary(self) -> Self {
        self.with_variant(ButtonVariant::Primary)
    }

    /// With the danger style for the Button.
    fn danger(self) -> Self {
        self.with_variant(ButtonVariant::Danger)
    }

    /// With the warning style for the Button.
    fn warning(self) -> Self {
        self.with_variant(ButtonVariant::Warning)
    }

    /// With the success style for the Button.
    fn success(self) -> Self {
        self.with_variant(ButtonVariant::Success)
    }

    /// With the info style for the Button.
    fn info(self) -> Self {
        self.with_variant(ButtonVariant::Info)
    }

    /// With the ghost style for the Button.
    fn ghost(self) -> Self {
        self.with_variant(ButtonVariant::Ghost)
    }

    /// With the link style for the Button.
    fn link(self) -> Self {
        self.with_variant(ButtonVariant::Link)
    }

    /// With the text style for the Button, it will no padding look like a normal text.
    fn text(self) -> Self {
        self.with_variant(ButtonVariant::Text)
    }

    /// With the custom style for the Button.
    fn custom(self, style: ButtonCustomVariant) -> Self {
        self.with_variant(ButtonVariant::Custom(style))
    }
}

impl ButtonCustomVariant {
    pub fn new(cx: &App) -> Self {
        Self {
            color: cx.theme().transparent,
            foreground: cx.theme().foreground,
            border: cx.theme().transparent,
            hover: cx.theme().transparent,
            active: cx.theme().transparent,
            shadow: false,
        }
    }

    /// Set background color, default is transparent.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    /// Set foreground color, default is theme foreground.
    pub fn foreground(mut self, color: Hsla) -> Self {
        self.foreground = color;
        self
    }

    /// Set border color, default is transparent.
    pub fn border(mut self, color: Hsla) -> Self {
        self.border = color;
        self
    }

    /// Set hover background color, default is transparent.
    pub fn hover(mut self, color: Hsla) -> Self {
        self.hover = color;
        self
    }

    /// Set active background color, default is transparent.
    pub fn active(mut self, color: Hsla) -> Self {
        self.active = color;
        self
    }

    /// Set shadow, default is false.
    pub fn shadow(mut self, shadow: bool) -> Self {
        self.shadow = shadow;
        self
    }
}

/// The veriant of the Button.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    Primary,
    #[default]
    Secondary,
    Danger,
    Info,
    Success,
    Warning,
    Ghost,
    Link,
    Text,
    Custom(ButtonCustomVariant),
}

impl ButtonVariant {
    #[inline]
    pub fn is_link(&self) -> bool {
        matches!(self, Self::Link)
    }

    #[inline]
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    #[inline]
    pub fn is_ghost(&self) -> bool {
        matches!(self, Self::Ghost)
    }

    #[inline]
    fn no_padding(&self) -> bool {
        self.is_link() || self.is_text()
    }
}

/// A Button element.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    base: Stateful<Div>,
    style: StyleRefinement,
    icon: Option<Icon>,
    label: Option<SharedString>,
    label_full_width: bool,
    accessible_label: Option<SharedString>,
    children: Vec<AnyElement>,
    disabled: bool,
    pub(crate) selected: bool,
    variant: ButtonVariant,
    rounded: ButtonRounded,
    outline: bool,
    border_corners: Corners<bool>,
    border_edges: Edges<bool>,
    dropdown_caret: bool,
    size: Size,
    compact: bool,
    wrapped_button: bool,
    wrapped_zoom: f32,
    wrapped_unit: Pixels,
    tooltip: Option<(
        SharedString,
        Option<(Rc<Box<dyn Action>>, Option<SharedString>)>,
    )>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_long_press: Option<Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>>,
    long_press_feedback: Option<Hsla>,
    interaction_scale: bool,
    action_icon_zoom: Option<f32>,
    interaction_transform: Option<(f32, f32, std::time::Duration, [f32; 4])>,
    on_hover: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
    state_style: Option<crate::StateStyle>,
    paint_motion: Option<ButtonPaintMotionStyle>,
    loading: bool,
    loading_icon: Option<Icon>,
    loading_icon_size: Option<Size>,

    focus_handle: Option<FocusHandle>,
    tab_index: isize,
    tab_stop: bool,
}

impl From<Button> for AnyElement {
    fn from(button: Button) -> Self {
        button.into_any_element()
    }
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();

        Self {
            id: id.clone(),
            // ID must be set after div is created;
            // `dropdown_menu` uses this id to create the popup menu.
            base: div().flex_shrink_0().id(id).role(gpui::Role::Button),
            style: StyleRefinement::default(),
            icon: None,
            label: None,
            label_full_width: false,
            accessible_label: None,
            disabled: false,
            selected: false,
            variant: ButtonVariant::default(),
            rounded: ButtonRounded::Medium,
            border_corners: Corners::all(true),
            border_edges: Edges::all(true),
            size: Size::Medium,
            tooltip: None,
            on_click: None,
            on_long_press: None,
            long_press_feedback: None,
            interaction_scale: false,
            action_icon_zoom: None,
            interaction_transform: None,
            on_hover: None,
            state_style: None,
            paint_motion: None,
            loading: false,
            compact: false,
            wrapped_button: false,
            wrapped_zoom: 1.,
            wrapped_unit: px(1.),
            outline: false,
            children: Vec::new(),
            loading_icon: None,
            loading_icon_size: None,
            dropdown_caret: false,
            focus_handle: None,
            tab_index: 0,
            tab_stop: true,
        }
    }

    /// Supply a retained handle for controls focused by a composite widget.
    pub fn focus_handle(mut self, handle: &FocusHandle) -> Self {
        self.focus_handle = Some(handle.clone());
        self
    }

    /// Use the application's wrapped Mantine button dimensions and typography.
    /// Explicit builder styles and theme CSS can still override these defaults.
    pub fn wrapped_button(mut self, radius: Pixels, zoom: f32, unit: Pixels) -> Self {
        self.wrapped_button = true;
        self.wrapped_zoom = zoom;
        self.wrapped_unit = unit;
        self.rounded = ButtonRounded::Size(radius);
        self.size = Size::Small;
        self
    }

    /// Animate the original circular play control's hover and press scale.
    pub fn interaction_scale(mut self) -> Self {
        self.interaction_scale = true;
        self
    }

    /// Match the original action icon's background fade, pressed offset and disabled opacity.
    /// The custom variant supplies its transparent/filled background and hover color.
    pub fn action_icon(mut self, zoom: f32) -> Self {
        self.action_icon_zoom = Some(zoom);
        self
    }

    /// Animate hover and pressed transforms using CSS timing, preserving explicit opacity.
    pub fn interaction_transform(
        mut self,
        hover: f32,
        pressed: f32,
        duration: std::time::Duration,
        curve: [f32; 4],
    ) -> Self {
        self.interaction_scale = true;
        self.interaction_transform = Some((hover, pressed, duration, curve));
        self
    }

    /// Set the outline style of the Button.
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    /// Set the border radius of the Button.
    pub fn rounded(mut self, rounded: impl Into<ButtonRounded>) -> Self {
        self.rounded = rounded.into();
        self
    }

    /// Set the border corners side of the Button.
    pub(crate) fn border_corners(mut self, corners: impl Into<Corners<bool>>) -> Self {
        self.border_corners = corners.into();
        self
    }

    /// Set the border edges of the Button.
    pub(crate) fn border_edges(mut self, edges: impl Into<Edges<bool>>) -> Self {
        self.border_edges = edges.into();
        self
    }

    /// Set label to the Button, if no label is set, the button will be in Icon Button mode.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Let custom label contents allocate the full inner button width.
    pub fn label_full_width(mut self) -> Self {
        self.label_full_width = true;
        self
    }

    /// Set the icon of the button, if the Button have no label, the button well in Icon Button mode.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set the tooltip of the button.
    /// Expose the state of a disclosure button to assistive technology.
    pub fn aria_expanded(mut self, expanded: bool) -> Self {
        self.base = self.base.aria_expanded(expanded);
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some((tooltip.into(), None));
        self
    }

    /// Remove a tooltip supplied by a shared button constructor.
    pub fn without_tooltip(mut self) -> Self {
        self.tooltip = None;
        self
    }

    /// Set the role when this control participates in a composite widget.
    pub fn role(mut self, role: gpui::Role) -> Self {
        self.base = self.base.role(role);
        self
    }

    /// Set a screen-reader label independently of visible text and tooltips.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// Set the tooltip of the button with action to show keybinding.
    pub fn tooltip_with_action(
        mut self,
        tooltip: impl Into<SharedString>,
        action: &dyn Action,
        context: Option<&str>,
    ) -> Self {
        self.tooltip = Some((
            tooltip.into(),
            Some((
                Rc::new(action.boxed_clone()),
                context.map(|c| c.to_string().into()),
            )),
        ));
        self
    }

    /// Set true to show the loading indicator.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Set the button to compact mode, then padding will be reduced.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Add click handler.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Run after a 500 ms primary-button hold. Release skips the normal click;
    /// leaving, disabling or removing the control cancels the pending hold.
    pub fn on_long_press(
        mut self,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_long_press = Some(Rc::new(handler));
        self
    }

    /// Show the original delayed expanding color circle while holding a play control.
    pub fn long_press_feedback(mut self, color: Hsla) -> Self {
        self.long_press_feedback = Some(color);
        self
    }

    /// Resolve custom CSS after variant, focus, disabled and selected styles.
    pub fn state_style(mut self, style: crate::StateStyle) -> Self {
        self.state_style = Some(style);
        self
    }

    pub fn paint_motion(mut self, motion: ButtonPaintMotionStyle) -> Self {
        self.paint_motion = Some(motion);
        self
    }

    /// Add hover handler, the bool parameter indicates whether the mouse is hovering.
    pub fn on_hover(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }

    /// Set the loading icon of the button, it will be used when loading is true.
    ///
    /// Default is a spinner icon.
    pub fn loading_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.loading_icon = Some(icon.into());
        self
    }

    /// Set the loading icon size independently of the button dimensions.
    pub fn loading_icon_size(mut self, size: impl Into<Size>) -> Self {
        self.loading_icon_size = Some(size.into());
        self
    }

    /// Set the tab index of the button, it will be used to focus the button by tab key.
    ///
    /// Default is 0.
    pub fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    /// Set the tab stop of the button, if true, the button will be focusable by tab key.
    ///
    /// Default is true.
    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// Set to show a dropdown caret icon at the end of the button.
    pub fn dropdown_caret(mut self, dropdown_caret: bool) -> Self {
        self.dropdown_caret = dropdown_caret;
        self
    }

    #[inline]
    fn clickable(&self) -> bool {
        !(self.disabled || self.loading) && self.on_click.is_some()
    }

    #[inline]
    fn hoverable(&self) -> bool {
        !(self.disabled || self.loading) && self.on_hover.is_some()
    }
}

impl Disableable for Button {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for Button {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl Sizable for Button {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl ButtonVariants for Button {
    fn with_variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl InteractiveElement for Button {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl gpui::StatefulInteractiveElement for Button {}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let style: ButtonVariant = self.variant;
        let clickable = self.clickable();
        let is_disabled = self.disabled || (self.wrapped_button && self.loading);
        let pointer_focus = self.wrapped_button;
        let press_disabled = self.disabled || self.loading;
        let hoverable = self.hoverable();
        let normal_style = style.normal(self.outline, cx);
        let icon_size = match self.size {
            Size::Size(v) => Size::Size(v * 0.75),
            _ => self.size,
        };

        let focus_handle = self.focus_handle.unwrap_or_else(|| {
            window
                .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone()
        });
        let long_press = self.on_long_press.as_ref().map(|handler| {
            let state =
                window.use_keyed_state(self.id.clone(), cx, |_, _| LongPressState::default());
            if press_disabled {
                state.update(cx, |state, cx| {
                    let was_pressed = state.pressed;
                    state.cancel();
                    if was_pressed {
                        cx.notify();
                    }
                });
            }
            (state, handler.clone())
        });
        let click_press = long_press.as_ref().map(|(state, _)| state.clone());
        let feedback = self.long_press_feedback.zip(click_press.clone());
        let interaction =
            (self.interaction_scale || self.action_icon_zoom.is_some() || self.wrapped_button)
                .then(|| {
                    let state = window.use_keyed_state(self.id.clone(), cx, |_, _| {
                        InteractionScaleState::new(press_disabled, self.interaction_transform)
                    });
                    state.update(cx, |state, cx| {
                        if state.disabled != press_disabled {
                            state.disabled = press_disabled;
                            if press_disabled {
                                state.pressed = false;
                            }
                            state.retarget(cx.background_executor().now());
                        }
                    });
                    state
                });
        let (scale, opacity) = interaction.as_ref().map_or((1., 1.), |state| {
            let state = state.read(cx);
            let now = cx.background_executor().now();
            let (scale, scaling) = state.scale.value(now);
            let (opacity, fading) = state.opacity.value(now);
            if scaling || fading {
                window.request_animation_frame();
            }
            (scale, opacity)
        });
        let is_focused = focus_handle.is_focused(window);
        let is_focus_visible = is_focused && (!self.wrapped_button || window.is_focus_visible());
        let action_paint = self.action_icon_zoom.map(|zoom| {
            let state = interaction.as_ref().unwrap();
            let (progress, animating, pressed) = state.update(cx, |state, cx| {
                let now = cx.background_executor().now();
                state
                    .background
                    .retarget(f32::from(state.hovered || state.pressed || is_focused), now);
                let (progress, animating) = state.background.value(now);
                (progress, animating, state.pressed && !press_disabled)
            });
            if animating {
                window.request_animation_frame();
            }
            let background = if normal_style.bg.a == 0. {
                style.hovered(self.outline, cx).bg.opacity(progress)
            } else {
                normal_style.bg
            };
            (background, px(if pressed { zoom } else { 0. }))
        });
        let mut states = Vec::new();
        states.push(if self.outline {
            "variant-outline"
        } else {
            match self.variant {
                ButtonVariant::Primary => "variant-filled",
                ButtonVariant::Secondary => "variant-default",
                ButtonVariant::Danger => "variant-state-error",
                ButtonVariant::Info => "variant-state-info",
                ButtonVariant::Success => "variant-state-success",
                ButtonVariant::Warning => "variant-state-warning",
                ButtonVariant::Ghost => "variant-subtle",
                ButtonVariant::Link | ButtonVariant::Text => "variant-transparent",
                ButtonVariant::Custom(_) => "variant-custom",
            }
        });
        if self.wrapped_button {
            states.push(match (self.size, self.compact) {
                (Size::XSmall, false) => "size-xs",
                (Size::XSmall, true) => "size-compact-xs",
                (Size::Small | Size::Size(_), false) => "size-sm",
                (Size::Small | Size::Size(_), true) => "size-compact-sm",
                (Size::Medium, false) => "size-md",
                (Size::Medium, true) => "size-compact-md",
                (Size::Large, false) => "size-lg",
                (Size::Large, true) => "size-compact-lg",
            });
            if self.icon.is_some() {
                states.push("with-left-section");
            }
        }
        if self.disabled {
            states.push("disabled");
        }
        if self.loading {
            states.push("loading");
        }
        if self.selected {
            states.push("checked");
        }
        if is_focused {
            states.push("focus");
        }
        if is_focus_visible {
            states.push("focus-visible");
        }
        // Hover text and layout must be resolved before shaping child text.
        // GPUI's paint-only hover refinement cannot recolor cached text runs.
        let normal_style = if self.wrapped_button {
            let interaction = interaction.as_ref().unwrap().read(cx);
            if interaction.hovered {
                states.push("hover");
            }
            if interaction.pressed {
                states.push("active");
            }
            if self.disabled {
                style.disabled(self.outline, cx)
            } else if interaction.pressed {
                style.active(self.outline, cx)
            } else if interaction.hovered {
                style.hovered(self.outline, cx)
            } else {
                normal_style
            }
        } else {
            normal_style
        };
        let resolve = |states: &[&str]| {
            self.state_style
                .as_ref()
                .map(|style| style(states, cx))
                .unwrap_or_default()
        };
        let wrapped_defaults = if self.wrapped_button {
            states.push("native-button-defaults");
            let resolved = resolve(&states);
            states.pop();
            // Resolve defaults independently of explicit builder dimensions. The
            // normal CSS cascade still applies after those builder refinements.
            let mut geometry = StyleRefinement::default();
            if !matches!(self.size, Size::Size(_)) {
                geometry.size.height = resolved.size.height;
            }
            geometry.padding = resolved.padding;
            geometry.corner_radii = resolved.corner_radii;
            geometry.gap = resolved.gap;
            geometry.text.get_or_insert_with(Default::default).font_size =
                resolved.text.and_then(|text| text.font_size);
            geometry
        } else {
            StyleRefinement::default()
        };
        let normal_css = resolve(&states);
        let paint_motion = self
            .paint_motion
            .as_ref()
            .and_then(|motion| motion(&states, cx));
        let wrapped_size = self.wrapped_button.then(|| {
            let zoom = f32::from(cx.theme().font_size) / 16.;
            let (height, padding, font) = match (self.size, self.compact) {
                (Size::XSmall, false) => (30., 14., 11.),
                (Size::XSmall, true) => (22., 7., 11.),
                (Size::Small, false) => (36., 18., 13.),
                (Size::Small, true) => (26., 8., 13.),
                (Size::Medium, false) => (42., 22., 14.),
                (Size::Medium, true) => (30., 10., 14.),
                (Size::Large, false) => (50., 26., 16.),
                (Size::Large, true) => (34., 12., 16.),
                (Size::Size(size), _) => (f32::from(size) / zoom, 18., 13.),
            };
            (
                px(height * zoom),
                px(padding * zoom),
                px(font * zoom),
                px(3. * zoom),
            )
        });
        let label_font_size = normal_css
            .text
            .as_ref()
            .and_then(|text| text.font_size)
            .or_else(|| self.style.text.as_ref().and_then(|text| text.font_size))
            .or_else(|| {
                wrapped_defaults
                    .text
                    .as_ref()
                    .and_then(|text| text.font_size)
            })
            .or_else(|| wrapped_size.map(|(_, _, size, _)| size.into()));
        states.push("hover");
        let hover_css = resolve(&states);
        states.push("active");
        let active_css = resolve(&states);

        let wrapped_paint = (self.wrapped_button && !self.selected).then(|| {
            let background_fill = normal_css
                .background
                .as_ref()
                .or(self.style.background.as_ref());
            let solid = background_fill
                .and_then(|fill| fill.color())
                .and_then(|background| background.as_solid());
            let background = solid.unwrap_or(normal_style.bg);
            let border = normal_css
                .border_color
                .or(self.style.border_color)
                .unwrap_or(normal_style.border);
            let foreground = normal_css
                .text
                .as_ref()
                .and_then(|text| text.color)
                .or_else(|| self.style.text.as_ref().and_then(|text| text.color))
                .unwrap_or(normal_style.fg);
            let opacity = normal_css.opacity.or(self.style.opacity).unwrap_or(1.);
            let paint = window.use_keyed_state(self.id.clone(), cx, |_, _| {
                WrappedPaint::new(
                    background,
                    border,
                    foreground,
                    opacity,
                    self.variant,
                    self.outline,
                    paint_motion,
                )
            });
            let (mut style, animating) = paint.update(cx, |paint, cx| {
                paint.update(
                    background,
                    border,
                    foreground,
                    opacity,
                    paint_motion,
                    cx.background_executor().now(),
                )
            });
            if background_fill.is_some() && solid.is_none() {
                style.background = None;
            }
            if animating {
                window.request_animation_frame();
            }
            style
        });

        let wrapped_loading = self.wrapped_button.then(|| {
            let state = window.use_keyed_state(self.id.clone(), cx, |_, _| {
                WrappedLoading::new(self.loading)
            });
            let (contents, loader, animating) = state.update(cx, |state, cx| {
                state.update(self.loading, cx.background_executor().now())
            });
            if animating {
                window.request_animation_frame();
            }
            (contents, loader)
        });
        let rounding = match self.rounded {
            ButtonRounded::Small => cx.theme().radius * 0.5,
            ButtonRounded::Medium => cx.theme().radius,
            ButtonRounded::Large => cx.theme().radius * 2.0,
            ButtonRounded::Size(px) => px,
            ButtonRounded::None => Pixels::ZERO,
        };

        let accessible_label = self.accessible_label.or_else(|| {
            self.tooltip
                .as_ref()
                .map(|value| value.0.clone())
                .or(self.label.clone())
        });
        self.base
            .aria_disabled(is_disabled)
            .aria_selected(self.selected)
            .when_some(accessible_label, |this, label| this.aria_label(label))
            .when(!is_disabled, |this| {
                this.track_focus(
                    &focus_handle
                        .tab_index(self.tab_index)
                        .tab_stop(self.tab_stop),
                )
            })
            .cursor_default()
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .cursor_default()
            .when(self.variant.is_link(), |this| this.cursor_pointer())
            .when(
                !self.wrapped_button && cx.theme().shadow && normal_style.shadow,
                |this| this.shadow_xs(),
            )
            .when(!style.no_padding(), |this| {
                if self.label.is_none() && self.children.is_empty() {
                    // Icon Button
                    match self.size {
                        Size::Size(px) => this.size(px),
                        Size::XSmall => this.size_5(),
                        Size::Small => this.size_6(),
                        Size::Large | Size::Medium => this.size_8(),
                    }
                } else {
                    // Normal Button
                    match self.size {
                        Size::Size(size) => this.px(size * 0.2),
                        Size::XSmall => this.h_5().px_1(),
                        Size::Small => this.h_6().px_3().when(self.compact, |this| this.px_1p5()),
                        _ => this.h_8().px_4().when(self.compact, |this| this.px_2()),
                    }
                }
            })
            .when(self.border_corners.top_left, |this| {
                this.rounded_tl(rounding)
            })
            .when(self.border_corners.top_right, |this| {
                this.rounded_tr(rounding)
            })
            .when(self.border_corners.bottom_left, |this| {
                this.rounded_bl(rounding)
            })
            .when(self.border_corners.bottom_right, |this| {
                this.rounded_br(rounding)
            })
            .when(self.border_edges.left, |this| this.border_l_1())
            .when(self.border_edges.right, |this| this.border_r_1())
            .when(self.border_edges.top, |this| this.border_t_1())
            .when(self.border_edges.bottom, |this| this.border_b_1())
            .text_color(normal_style.fg)
            .when(self.selected, |this| {
                let selected_style = style.selected(self.outline, cx);
                this.bg(selected_style.bg)
                    .border_color(selected_style.border)
                    .text_color(selected_style.fg)
            })
            .when(
                !self.disabled && !self.selected && action_paint.is_none(),
                |this| {
                    this.border_color(normal_style.border)
                        .bg(normal_style.bg)
                        .when(normal_style.underline, |this| this.text_decoration_1())
                        .when(!self.wrapped_button, |this| {
                            this.hover(|this| {
                                let hover_style = style.hovered(self.outline, cx);
                                this.bg(hover_style.bg)
                                    .border_color(hover_style.border)
                                    .text_color(hover_style.fg)
                                    .refine_style(&hover_css)
                            })
                            .active(|this| {
                                let active_style = style.active(self.outline, cx);
                                this.bg(active_style.bg)
                                    .border_color(active_style.border)
                                    .text_color(active_style.fg)
                                    .refine_style(&active_css)
                            })
                        })
                },
            )
            .when(self.disabled, |this| {
                let disabled_style = style.disabled(self.outline, cx);
                this.bg(disabled_style.bg)
                    .text_color(disabled_style.fg)
                    .border_color(disabled_style.border)
                    .shadow_none()
            })
            .when(
                !self.wrapped_button && (self.disabled || self.selected) && action_paint.is_none(),
                |this| this.hover(|style| style.refine_style(&hover_css)),
            )
            .when(
                !self.wrapped_button && self.selected && !self.disabled && action_paint.is_none(),
                |this| this.active(|style| style.refine_style(&active_css)),
            )
            .when_some(action_paint, |this, (background, offset)| {
                this.bg(background)
                    .text_color(normal_style.fg)
                    .border_color(normal_style.border)
                    .opacity(if is_disabled { 0.6 } else { 1. })
                    .relative()
                    .top(offset)
                    .hover(|style| style.refine_style(&hover_css))
                    .active(|style| style.refine_style(&active_css))
            })
            .when_some(wrapped_size, |this, (height, padding, _, _)| {
                this.h(height)
                    .px(padding)
                    .when(self.icon.is_some(), |this| this.pl(padding / 1.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .cursor_pointer()
                    .when(is_disabled, |this| {
                        this.cursor(gpui::CursorStyle::OperationNotAllowed)
                    })
            })
            .refine_style(&wrapped_defaults)
            .refine_style(&self.style)
            .paint_scale(if self.interaction_scale { scale } else { 1. })
            .when(
                self.interaction_scale && self.interaction_transform.is_none(),
                |this| {
                    this.opacity(opacity).when(press_disabled, |this| {
                        this.bg(normal_style.bg).text_color(normal_style.fg)
                    })
                },
            )
            .refine_style(&normal_css)
            .when_some(wrapped_paint, |this, style| this.refine_style(&style))
            .when_some(interaction.clone(), |this, state| {
                let inside_release = state.clone();
                let outside_release = state.clone();
                this.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    if press_disabled {
                        return;
                    }
                    state.update(cx, |state, cx| {
                        state.pressed = true;
                        state.retarget(cx.background_executor().now());
                        cx.notify();
                    });
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    if press_disabled {
                        return;
                    }
                    inside_release.update(cx, |state, cx| {
                        state.pressed = false;
                        state.hovered = true;
                        state.retarget(cx.background_executor().now());
                        cx.notify();
                    });
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    outside_release.update(cx, |state, cx| {
                        state.pressed = false;
                        state.hovered = false;
                        state.retarget(cx.background_executor().now());
                        cx.notify();
                    });
                })
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                // Stop handle any click event when disabled.
                // To avoid handle dropdown menu open when button is disabled.
                if is_disabled {
                    if pointer_focus {
                        window.blur();
                    }
                    cx.stop_propagation();
                    return;
                }

                // Wrapped application buttons follow the browser's pointer focus behavior.
                if !pointer_focus {
                    window.prevent_default();
                }
            })
            .when_some(long_press, |this, (state, handler)| {
                let inside_release_state = state.clone();
                let release_state = state.clone();
                let exit_state = state.clone();
                this.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    if press_disabled {
                        return;
                    }
                    let event = event.clone();
                    let handler = handler.clone();
                    state.update(cx, |state, cx| {
                        state.pressed = true;
                        state.canceled = false;
                        state.fired = false;
                        state.started = Some(cx.background_executor().now());
                        cx.notify();
                        let timer = cx
                            .background_executor()
                            .timer(std::time::Duration::from_millis(500));
                        state.timer = Some(cx.spawn_in(window, async move |state, cx| {
                            timer.await;
                            state
                                .update_in(cx, |state, window, cx| {
                                    if state.pressed
                                        && !state.canceled
                                        && window.is_keyed_state_present(&cx.entity())
                                    {
                                        state.fired = true;
                                        state.started = None;
                                        cx.notify();
                                        handler(&event, window, cx);
                                    } else {
                                        state.cancel();
                                    }
                                })
                                .ok();
                        }));
                    });
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    inside_release_state.update(cx, |state, cx| {
                        state.timer = None;
                        state.pressed = false;
                        state.started = None;
                        cx.notify();
                    });
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    release_state.update(cx, |state, cx| {
                        state.cancel();
                        cx.notify();
                    });
                })
                .on_mouse_move_out(move |_, _, cx| {
                    exit_state.update(cx, |state, cx| {
                        if state.pressed {
                            state.cancel();
                            cx.notify();
                        }
                    });
                })
            })
            .when_some(self.on_click, |this, on_click| {
                this.on_click(move |event, window, cx| {
                    // Stop handle any click event when disabled.
                    // To avoid handle dropdown menu open when button is disabled.
                    if !clickable {
                        cx.stop_propagation();
                        return;
                    }

                    if let Some(state) = &click_press {
                        let skip = state.update(cx, |state, cx| {
                            let skip = matches!(event, ClickEvent::Mouse(_))
                                && (state.fired || state.canceled);
                            state.timer = None;
                            state.pressed = false;
                            state.canceled = false;
                            state.fired = false;
                            state.started = None;
                            cx.notify();
                            skip
                        });
                        if skip {
                            cx.stop_propagation();
                            return;
                        }
                    }
                    (on_click)(event, window, cx);
                })
            })
            .when(hoverable || interaction.is_some(), |this| {
                let on_hover = self.on_hover;
                this.on_hover(move |hovered, window, cx| {
                    if let Some(state) = &interaction {
                        state.update(cx, |state, cx| {
                            state.hovered = *hovered;
                            state.retarget(cx.background_executor().now());
                            cx.notify();
                        });
                    }
                    if !press_disabled {
                        if let Some(on_hover) = &on_hover {
                            on_hover(hovered, window, cx);
                        }
                    }
                })
            })
            .when_some(feedback, |this, (color, state)| {
                this.relative().child(
                    gpui::canvas(
                        |_, _, _| {},
                        move |bounds, _, window, cx| {
                            let Some(started) = state.read(cx).started else {
                                return;
                            };
                            let elapsed = cx
                                .background_executor()
                                .now()
                                .saturating_duration_since(started)
                                .as_secs_f32();
                            if elapsed < 0.4 {
                                window.request_animation_frame();
                            }
                            let progress = ((elapsed - 0.1) / 0.3).clamp(0., 1.);
                            if progress == 0. {
                                return;
                            }
                            let diameter =
                                bounds.size.width.min(bounds.size.height) * (1.05 * progress);
                            let size = gpui::size(diameter, diameter);
                            let bounds = gpui::Bounds::new(
                                bounds.center() - gpui::point(diameter / 2., diameter / 2.),
                                size,
                            );
                            let mut color = color;
                            color.a *= 0.2 + 0.6 * progress;
                            let mut quad = gpui::fill(bounds, color);
                            quad.corner_radii = Corners::all(diameter / 2.);
                            window.paint_quad(quad);
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
            })
            .child({
                h_flex()
                    .id("label")
                    .when(self.label_full_width, |this| this.w_full())
                    .when_some(wrapped_loading, |this, (contents, _)| {
                        this.opacity(contents)
                    })
                    .items_center()
                    .justify_center()
                    .button_text_size(self.size)
                    .when_some(label_font_size, |label, size| label.text_size(size))
                    .map(|this| match self.size {
                        Size::XSmall => this.gap_1(),
                        Size::Small => this.gap_1(),
                        _ => this.gap_2(),
                    })
                    .when(self.wrapped_button, |this| {
                        this.gap(
                            wrapped_defaults
                                .gap
                                .width
                                .unwrap_or((cx.theme().font_size * 0.5).into()),
                        )
                    })
                    .when(!self.loading || self.wrapped_button, |this| {
                        this.when_some(self.icon.clone(), |this, icon| {
                            this.child(icon.with_default_size(icon_size))
                        })
                    })
                    .when(self.loading && !self.wrapped_button, |this| {
                        this.child(
                            Spinner::new()
                                .with_size(self.loading_icon_size.unwrap_or(self.size))
                                .when(self.interaction_scale, |spinner| {
                                    spinner
                                        .animation(std::time::Duration::from_secs(1), gpui::linear)
                                })
                                .when_some(self.loading_icon.clone(), |this, icon| this.icon(icon)),
                        )
                    })
                    .when_some(self.label, |this, label| {
                        this.child(
                            div()
                                .flex_none()
                                .line_height(relative(1.))
                                .when(self.wrapped_button && self.loading, |this| {
                                    this.opacity(0.2)
                                })
                                .child(label),
                        )
                    })
                    .when(!(self.loading && self.interaction_scale), |this| {
                        if self.wrapped_button && !self.children.is_empty() {
                            this.child(
                                h_flex()
                                    .when(self.loading, |this| this.opacity(0.2))
                                    .children(self.children),
                            )
                        } else {
                            this.children(self.children)
                        }
                    })
                    .when(self.dropdown_caret, |this| {
                        this.child(Icon::new(IconName::ChevronDown).with_size(icon_size))
                    })
            })
            .when(
                self.loading && !self.disabled && !self.interaction_scale && !self.wrapped_button,
                |this| {
                    this.bg(normal_style.bg.opacity(0.8))
                        .border_color(normal_style.border.opacity(0.8))
                        .text_color(normal_style.fg.opacity(0.8))
                },
            )
            .when_some(wrapped_loading, |this, (contents, loader)| {
                if contents == 1. && loader == 0. && !self.loading {
                    return this;
                }
                let spinner_size = match self
                    .loading_icon_size
                    .unwrap_or(Size::Size(cx.theme().font_size * 1.125))
                {
                    Size::Size(size) => size,
                    _ => cx.theme().font_size * 1.125,
                };
                let font_size = label_font_size
                    .map(|size| size.to_pixels(window.rem_size()))
                    .unwrap_or(cx.theme().font_size);
                let mut text = window.text_style();
                text.font_weight = gpui::FontWeight::MEDIUM;
                if let Some(style) = &self.style.text {
                    text.refine(style);
                }
                if let Some(style) = &normal_css.text {
                    text.refine(style);
                }
                let font = window.text_system().resolve_font(&text.font());
                let below = ((font_size - window.text_system().ascent(font, font_size)
                    + window.text_system().descent(font, font_size).abs())
                    / 2.)
                    .max(Pixels::ZERO);
                let above = (font_size - below).max(spinner_size);
                let line_height = above + below;
                let offset =
                    -line_height * 2. + (line_height * 1.5 + self.wrapped_unit) * loader + above
                        - spinner_size;
                let mut corners = Corners::all(rounding);
                for source in [&wrapped_defaults, &self.style, &normal_css] {
                    if let Some(value) = source.corner_radii.top_left {
                        corners.top_left = value.to_pixels(window.rem_size());
                    }
                    if let Some(value) = source.corner_radii.top_right {
                        corners.top_right = value.to_pixels(window.rem_size());
                    }
                    if let Some(value) = source.corner_radii.bottom_left {
                        corners.bottom_left = value.to_pixels(window.rem_size());
                    }
                    if let Some(value) = source.corner_radii.bottom_right {
                        corners.bottom_right = value.to_pixels(window.rem_size());
                    }
                }
                let background = if cx.theme().mode.is_dark() {
                    gpui::black()
                } else {
                    gpui::white()
                }
                .opacity(0.15 * (1. - contents));
                let loading = self.loading;
                let inset = self.wrapped_unit;
                this.relative().child(
                    div()
                        .absolute()
                        .inset_0()
                        .overflow_hidden()
                        .rounded_tl(corners.top_left)
                        .rounded_tr(corners.top_right)
                        .rounded_bl(corners.bottom_left)
                        .rounded_br(corners.bottom_right)
                        .child(
                            gpui::canvas(
                                |_, _, _| {},
                                move |bounds, _, window, _| {
                                    let mut bounds = bounds.dilate(inset);
                                    if !loading {
                                        bounds.origin.y -= bounds.size.height;
                                    }
                                    let mut quad = gpui::fill(bounds, background);
                                    quad.corner_radii = corners;
                                    window.paint_quad(quad);
                                },
                            )
                            .absolute()
                            .size_full()
                            .foreground_blur(px(12. * self.wrapped_zoom)),
                        )
                        .when(loader > 0. || self.loading, |this| {
                            this.child(
                                div()
                                    .debug_selector(|| "wrapped-button-loader".into())
                                    .absolute()
                                    .left(relative(0.5))
                                    .top(relative(0.5))
                                    .ml(-spinner_size / 2.)
                                    .mt(offset)
                                    .opacity(loader)
                                    .child(
                                        Spinner::new()
                                            .with_size(spinner_size)
                                            .animation(
                                                std::time::Duration::from_secs(1),
                                                gpui::linear,
                                            )
                                            .when_some(
                                                self.loading_icon.clone(),
                                                |spinner, icon| spinner.icon(icon),
                                            ),
                                    ),
                            )
                        }),
                )
            })
            .when_some(self.tooltip, |this, (tooltip, action)| {
                this.tooltip(move |window, cx| {
                    Tooltip::new(tooltip.clone())
                        .when_some(action.clone(), |this, (action, context)| {
                            this.action(
                                action.boxed_clone().as_ref(),
                                context.as_ref().map(|c| c.as_ref()),
                            )
                        })
                        .build(window, cx)
                })
            })
            .when(
                self.wrapped_button
                    && is_focus_visible
                    && self.variant == ButtonVariant::Primary
                    && !self.outline,
                |this| {
                    let zoom = self.wrapped_zoom;
                    let width = px(zoom);
                    let inset = px(3. * zoom)
                        + self
                            .style
                            .border_widths
                            .top
                            .unwrap_or(px(1.).into())
                            .to_pixels(window.rem_size());
                    this.relative().child(
                        div()
                            .absolute()
                            .top(-inset)
                            .left(-inset)
                            .bottom(-inset)
                            .right(-inset)
                            .border(width)
                            .border_color(cx.theme().primary)
                            .rounded(rounding + px(3. * zoom)),
                    )
                },
            )
            .focus_ring(
                is_focused && self.action_icon_zoom.is_none() && !self.wrapped_button,
                px(0.),
                window,
                cx,
            )
    }
}

struct ButtonVariantStyle {
    bg: Hsla,
    border: Hsla,
    fg: Hsla,
    underline: bool,
    shadow: bool,
}

impl ButtonVariant {
    fn bg_color(&self, outline: bool, cx: &mut App) -> Hsla {
        if outline {
            return cx.theme().background;
        }

        match self {
            ButtonVariant::Primary => cx.theme().primary,
            ButtonVariant::Secondary => cx.theme().secondary,
            ButtonVariant::Danger => cx.theme().danger,
            ButtonVariant::Warning => cx.theme().warning,
            ButtonVariant::Success => cx.theme().success,
            ButtonVariant::Info => cx.theme().info,
            ButtonVariant::Ghost | ButtonVariant::Link | ButtonVariant::Text => {
                cx.theme().transparent
            }
            ButtonVariant::Custom(colors) => colors.color,
        }
    }

    fn text_color(&self, outline: bool, cx: &mut App) -> Hsla {
        match self {
            ButtonVariant::Primary => {
                if outline {
                    cx.theme().primary
                } else {
                    cx.theme().primary_foreground
                }
            }
            ButtonVariant::Secondary | ButtonVariant::Ghost => cx.theme().secondary_foreground,
            ButtonVariant::Danger => {
                if outline {
                    cx.theme().danger
                } else {
                    cx.theme().danger_foreground
                }
            }
            ButtonVariant::Warning => {
                if outline {
                    cx.theme().warning
                } else {
                    cx.theme().warning_foreground
                }
            }
            ButtonVariant::Success => {
                if outline {
                    cx.theme().success
                } else {
                    cx.theme().success_foreground
                }
            }
            ButtonVariant::Info => {
                if outline {
                    cx.theme().info
                } else {
                    cx.theme().info_foreground
                }
            }
            ButtonVariant::Link => cx.theme().link,
            ButtonVariant::Text => cx.theme().foreground,
            ButtonVariant::Custom(colors) => {
                if outline {
                    colors.color
                } else {
                    colors.foreground
                }
            }
        }
    }

    fn border_color(&self, bg: Hsla, outline: bool, cx: &mut App) -> Hsla {
        match self {
            ButtonVariant::Secondary => {
                if outline {
                    cx.theme().border
                } else {
                    bg
                }
            }
            ButtonVariant::Primary => {
                if outline {
                    cx.theme().primary
                } else {
                    bg
                }
            }
            ButtonVariant::Danger => {
                if outline {
                    cx.theme().danger
                } else {
                    bg
                }
            }
            ButtonVariant::Info => {
                if outline {
                    cx.theme().info
                } else {
                    bg
                }
            }
            ButtonVariant::Warning => {
                if outline {
                    cx.theme().warning
                } else {
                    bg
                }
            }
            ButtonVariant::Success => {
                if outline {
                    cx.theme().success
                } else {
                    bg
                }
            }
            ButtonVariant::Ghost | ButtonVariant::Link | ButtonVariant::Text => {
                cx.theme().transparent
            }
            ButtonVariant::Custom(colors) => colors.border,
        }
    }

    fn underline(&self, _: &App) -> bool {
        match self {
            ButtonVariant::Link => true,
            _ => false,
        }
    }

    fn shadow(&self, outline: bool, _: &App) -> bool {
        match self {
            ButtonVariant::Primary | ButtonVariant::Secondary | ButtonVariant::Danger => outline,
            ButtonVariant::Custom(c) => c.shadow,
            _ => false,
        }
    }

    fn normal(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg = self.bg_color(outline, cx);
        let border = self.border_color(bg, outline, cx);
        let fg = self.text_color(outline, cx);
        let underline = self.underline(cx);
        let shadow = self.shadow(outline, cx);

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }

    fn hovered(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg = match self {
            ButtonVariant::Primary => {
                if outline {
                    cx.theme().secondary_hover
                } else {
                    cx.theme().primary_hover
                }
            }
            ButtonVariant::Secondary => cx.theme().secondary_hover,
            ButtonVariant::Danger => {
                if outline {
                    cx.theme().secondary_hover
                } else {
                    cx.theme().danger_hover
                }
            }
            ButtonVariant::Warning => {
                if outline {
                    cx.theme().secondary_hover
                } else {
                    cx.theme().warning_hover
                }
            }
            ButtonVariant::Success => {
                if outline {
                    cx.theme().secondary_hover
                } else {
                    cx.theme().success_hover
                }
            }
            ButtonVariant::Info => {
                if outline {
                    cx.theme().secondary_hover
                } else {
                    cx.theme().info_hover
                }
            }
            ButtonVariant::Ghost => {
                if cx.theme().mode.is_dark() {
                    cx.theme().secondary.lighten(0.1).opacity(0.8)
                } else {
                    cx.theme().secondary.darken(0.1).opacity(0.8)
                }
            }
            ButtonVariant::Link => cx.theme().transparent,
            ButtonVariant::Text => cx.theme().transparent,
            ButtonVariant::Custom(colors) => {
                if outline {
                    cx.theme().secondary_hover
                } else {
                    colors.hover
                }
            }
        };

        let border = self.border_color(bg, outline, cx);
        let fg = match self {
            ButtonVariant::Link => cx.theme().link_hover,
            _ => self.text_color(outline, cx),
        };

        let underline = self.underline(cx);
        let shadow = self.shadow(outline, cx);

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }

    fn active(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg = match self {
            ButtonVariant::Primary => {
                if outline {
                    cx.theme().primary_active.opacity(0.1)
                } else {
                    cx.theme().primary_active
                }
            }
            ButtonVariant::Secondary => cx.theme().secondary_active,
            ButtonVariant::Ghost => {
                if cx.theme().mode.is_dark() {
                    cx.theme().secondary.lighten(0.2).opacity(0.8)
                } else {
                    cx.theme().secondary.darken(0.2).opacity(0.8)
                }
            }
            ButtonVariant::Danger => {
                if outline {
                    cx.theme().danger_active.opacity(0.1)
                } else {
                    cx.theme().danger_active
                }
            }
            ButtonVariant::Warning => {
                if outline {
                    cx.theme().warning_active.opacity(0.1)
                } else {
                    cx.theme().warning_active
                }
            }
            ButtonVariant::Success => {
                if outline {
                    cx.theme().success_active.opacity(0.1)
                } else {
                    cx.theme().success_active
                }
            }
            ButtonVariant::Info => {
                if outline {
                    cx.theme().info_active.opacity(0.1)
                } else {
                    cx.theme().info_active
                }
            }
            ButtonVariant::Link => cx.theme().transparent,
            ButtonVariant::Text => cx.theme().transparent,
            ButtonVariant::Custom(colors) => {
                if outline {
                    colors.active.opacity(0.1)
                } else {
                    colors.active
                }
            }
        };
        let border = self.border_color(bg, outline, cx);
        let fg = match self {
            ButtonVariant::Link => cx.theme().link_active,
            ButtonVariant::Text => cx.theme().foreground.opacity(0.7),
            _ => self.text_color(outline, cx),
        };
        let underline = self.underline(cx);
        let shadow = self.shadow(outline, cx);

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }

    fn selected(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg = match self {
            ButtonVariant::Primary => cx.theme().primary_active,
            ButtonVariant::Secondary | ButtonVariant::Ghost => cx.theme().secondary_active,
            ButtonVariant::Danger => cx.theme().danger_active,
            ButtonVariant::Warning => cx.theme().warning_active,
            ButtonVariant::Success => cx.theme().success_active,
            ButtonVariant::Info => cx.theme().info_active,
            ButtonVariant::Link => cx.theme().transparent,
            ButtonVariant::Text => cx.theme().transparent,
            ButtonVariant::Custom(colors) => colors.active,
        };

        let border = self.border_color(bg, outline, cx);
        let fg = match self {
            ButtonVariant::Link => cx.theme().link_active,
            ButtonVariant::Text => cx.theme().foreground.opacity(0.7),
            _ => self.text_color(false, cx),
        };
        let underline = self.underline(cx);
        let shadow = self.shadow(outline, cx);

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }

    fn disabled(&self, outline: bool, cx: &mut App) -> ButtonVariantStyle {
        let bg = match self {
            ButtonVariant::Link | ButtonVariant::Ghost | ButtonVariant::Text => {
                cx.theme().transparent
            }
            ButtonVariant::Primary => cx.theme().primary.opacity(0.15),
            ButtonVariant::Danger => cx.theme().danger.opacity(0.15),
            ButtonVariant::Warning => cx.theme().warning.opacity(0.15),
            ButtonVariant::Success => cx.theme().success.opacity(0.15),
            ButtonVariant::Info => cx.theme().info.opacity(0.15),
            ButtonVariant::Secondary => cx.theme().secondary.opacity(1.5),
            ButtonVariant::Custom(style) => style.color.opacity(0.15),
        };
        let fg = cx.theme().muted_foreground.opacity(0.5);
        let (bg, border) = if outline {
            (cx.theme().transparent, cx.theme().border.opacity(0.5))
        } else {
            (bg, bg)
        };

        let underline = self.underline(cx);
        let shadow = false;

        ButtonVariantStyle {
            bg,
            border,
            fg,
            underline,
            shadow,
        }
    }
}
