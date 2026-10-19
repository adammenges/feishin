use gpui::{
    AnyElement, App, Axis, Bounds, ClickEvent, Context, Corner, DismissEvent, ElementId, Entity,
    EventEmitter, FocusHandle, Focusable, Global, InteractiveElement as _, IntoElement, KeyBinding,
    MouseButton, ParentElement, Pixels, Point, Render, RenderOnce, StyleRefinement, Styled,
    Subscription, WeakEntity, Window, anchored, canvas, deferred, div, prelude::FluentBuilder as _,
    px,
};
use std::rc::Rc;

use crate::{Selectable, StyledExt as _, actions::Cancel, animation::CssTransition, v_flex};

const CONTEXT: &str = "Popover";
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Cancel, Some(CONTEXT))])
}

/// A popover element that can be triggered by a button or any other element.
#[derive(IntoElement)]
pub struct Popover {
    id: ElementId,
    style: StyleRefinement,
    anchor: Corner,
    center_axis: Option<Axis>,
    side: Option<Corner>,
    offset: Option<Pixels>,
    transition_offset: Option<Point<Pixels>>,
    viewport_margin: Pixels,
    default_open: bool,
    open: Option<bool>,
    tracked_focus_handle: Option<FocusHandle>,
    trigger:
        Option<Box<dyn FnOnce(bool, Entity<PopoverState>, &Window, &App) -> AnyElement + 'static>>,
    content: Option<
        Rc<
            dyn Fn(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> AnyElement
                + 'static,
        >,
    >,
    children: Vec<AnyElement>,
    /// Style for trigger element.
    /// This is used for hotfix the trigger element style to support w_full.
    trigger_style: Option<StyleRefinement>,
    mouse_button: MouseButton,
    appearance: bool,
    overlay_closable: bool,
    defer_content: bool,
    on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
}

impl Popover {
    /// Create a new Popover with `view` mode.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: StyleRefinement::default(),
            anchor: Corner::TopLeft,
            center_axis: None,
            side: None,
            offset: None,
            transition_offset: None,
            viewport_margin: px(8.),
            trigger: None,
            trigger_style: None,
            content: None,
            tracked_focus_handle: None,
            children: vec![],
            mouse_button: MouseButton::Left,
            appearance: true,
            overlay_closable: true,
            defer_content: true,
            default_open: false,
            open: None,
            on_open_change: None,
        }
    }

    /// Set the anchor corner of the popover, default is `Corner::TopLeft`.
    pub fn anchor(mut self, anchor: Corner) -> Self {
        self.anchor = anchor;
        self
    }

    /// Center the popup on its trigger along this axis.
    pub fn centered(mut self, axis: Axis) -> Self {
        self.center_axis = Some(axis);
        self
    }

    /// Place beside the trigger, using the given popup corner for alignment.
    pub fn side(mut self, anchor: Corner) -> Self {
        self.side = Some(anchor);
        self
    }

    /// Set the distance from the trigger in logical pixels.
    pub fn offset(mut self, offset: Pixels) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Fade and translate in and out using the original 150 ms CSS ease transition.
    pub fn transition_offset(mut self, offset: Point<Pixels>) -> Self {
        self.transition_offset = Some(offset);
        self
    }

    /// Keep this much space between centered popup content and the viewport edge.
    pub fn viewport_margin(mut self, margin: Pixels) -> Self {
        self.viewport_margin = margin;
        self
    }

    /// Set the mouse button to trigger the popover, default is `MouseButton::Left`.
    pub fn mouse_button(mut self, mouse_button: MouseButton) -> Self {
        self.mouse_button = mouse_button;
        self
    }

    /// Set the trigger element of the popover.
    pub fn trigger<T>(mut self, trigger: T) -> Self
    where
        T: Selectable + gpui::StatefulInteractiveElement + IntoElement + 'static,
    {
        self.trigger = Some(Box::new(|is_open, state, window, _| {
            let selected = trigger.is_selected();
            let parent_view = window.current_view();
            trigger
                .selected(selected || is_open)
                .aria_expanded(is_open)
                .on_click(move |event, window, cx| {
                    // Pointer presses open through the wrapper below. Keyboard and
                    // accessibility activation target the trigger directly.
                    if matches!(event, ClickEvent::Keyboard(_)) {
                        state.update(cx, |state, cx| state.toggle_open(window, cx));
                        cx.notify(parent_view);
                    }
                })
                .into_any_element()
        }));
        self
    }

    /// Anchor to an element that has no selected state, such as an editable row.
    pub fn trigger_element(mut self, trigger: impl IntoElement + 'static) -> Self {
        self.trigger = Some(Box::new(|_, _, _, _| trigger.into_any_element()));
        self
    }

    /// Set the default open state of the popover, default is `false`.
    ///
    /// This is only used to initialize the open state of the popover.
    ///
    /// And please note that if you use the `open` method, this value will be ignored.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Force set the open state of the popover.
    ///
    /// If this is set, the popover will be controlled by this value.
    ///
    /// NOTE: You must be used in conjunction with `on_open_change` to handle state changes.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Add a callback to be called when the open state changes.
    ///
    /// The first `&bool` parameter is the **new open state**.
    ///
    /// This is useful when using the `open` method to control the popover state.
    pub fn on_open_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&bool, &mut Window, &mut App) + 'static,
    {
        self.on_open_change = Some(Rc::new(callback));
        self
    }

    /// Set the style for the trigger element.
    pub fn trigger_style(mut self, style: StyleRefinement) -> Self {
        self.trigger_style = Some(style);
        self
    }

    /// Set whether clicking outside the popover will dismiss it, default is `true`.
    pub fn overlay_closable(mut self, closable: bool) -> Self {
        self.overlay_closable = closable;
        self
    }

    /// Paint in place when this is already a foreground surface that contains
    /// deferred menus. GPUI does not permit a deferred draw inside another one.
    pub fn defer_content(mut self, defer: bool) -> Self {
        self.defer_content = defer;
        self
    }

    /// Set the content builder for content of the Popover.
    ///
    /// This callback will called every time on render the popover.
    /// So, you should avoid creating new elements or entities in the content closure.
    pub fn content<F, E>(mut self, content: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> E + 'static,
    {
        self.content = Some(Rc::new(move |state, window, cx| {
            content(state, window, cx).into_any_element()
        }));
        self
    }

    /// Set whether the popover no style, default is `false`.
    ///
    /// If no style:
    ///
    /// - The popover will not have a bg, border, shadow, or padding.
    /// - The click out of the popover will not dismiss it.
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    /// Bind the focus handle to receive focus when the popover is opened.
    /// If you not set this, a new focus handle will be created for the popover to
    ///
    /// If popover is opened, the focus will be moved to the focus handle.
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.tracked_focus_handle = Some(handle.clone());
        self
    }

    fn resolved_corner(anchor: Corner, bounds: Bounds<Pixels>) -> Point<Pixels> {
        bounds.corner(match anchor {
            Corner::TopLeft => Corner::BottomLeft,
            Corner::TopRight => Corner::BottomRight,
            Corner::BottomLeft => Corner::TopLeft,
            Corner::BottomRight => Corner::TopRight,
        }) + Point {
            x: px(0.),
            y: -bounds.size.height,
        }
    }
}

impl ParentElement for Popover {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for Popover {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[derive(Default)]
struct OpenPopovers(Vec<WeakEntity<PopoverState>>);
impl Global for OpenPopovers {}

pub struct PopoverState {
    focus_handle: FocusHandle,
    previous_focus: Option<FocusHandle>,
    pub(crate) tracked_focus_handle: Option<FocusHandle>,
    trigger_bounds: Option<Bounds<Pixels>>,
    content_bounds: Option<Bounds<Pixels>>,
    open: bool,
    transition: CssTransition,
    on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,

    _dismiss_subscription: Option<Subscription>,
    _focus_lost_subscription: Option<Subscription>,
}

impl PopoverState {
    pub fn new(default_open: bool, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            previous_focus: None,
            tracked_focus_handle: None,
            trigger_bounds: None,
            content_bounds: None,
            open: default_open,
            transition: CssTransition::new(
                f32::from(default_open),
                std::time::Duration::from_millis(150),
                [0.25, 0.1, 0.25, 1.],
            ),
            on_open_change: None,
            _dismiss_subscription: None,
            _focus_lost_subscription: None,
        }
    }

    /// Check if the popover is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Dismiss the popover if it is open.
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.toggle_open(window, cx);
        }
    }

    /// Open the popover if it is closed.
    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            self.toggle_open(window, cx);
        }
    }

    fn toggle_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = !self.open;
        if self.open {
            self.previous_focus = window.focused(cx);
            let state = cx.entity();
            let focus_handle = if let Some(tracked_focus_handle) = self.tracked_focus_handle.clone()
            {
                tracked_focus_handle
            } else {
                self.focus_handle.clone()
            };
            focus_handle.focus(window);

            self._dismiss_subscription =
                Some(
                    window.subscribe(&cx.entity(), cx, move |_, _: &DismissEvent, window, cx| {
                        state.update(cx, |state, cx| {
                            state.dismiss(window, cx);
                        });
                        window.refresh();
                    }),
                );
        } else {
            self._dismiss_subscription = None;
            let owns_focus = self.focus_handle.contains_focused(window, cx)
                || self
                    .tracked_focus_handle
                    .as_ref()
                    .is_some_and(|focus| focus.contains_focused(window, cx));
            if let Some(previous) = self.previous_focus.take()
                && owns_focus
            {
                previous.focus(window);
            }
        }

        if let Some(callback) = self.on_open_change.as_ref() {
            callback(&self.open, window, cx);
        }
        cx.notify();
    }

    fn on_action_cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss(window, cx);
    }
}

impl Focusable for PopoverState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PopoverState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl EventEmitter<DismissEvent> for PopoverState {}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let force_open = self.open;
        let default_open = self.default_open;
        let tracked_focus_handle = self.tracked_focus_handle.clone();
        let state = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            let weak = cx.weak_entity();
            if cx.try_global::<OpenPopovers>().is_none() {
                cx.set_global(OpenPopovers::default());
            }
            let registry = cx.global_mut::<OpenPopovers>();
            registry.0.retain(|state| state.upgrade().is_some());
            registry.0.push(weak);
            let mut state = PopoverState::new(default_open, cx);
            state._focus_lost_subscription = Some(cx.on_focus_lost(window, |state, window, cx| {
                // A disabled child clears focus like a browser button. Keep Escape and
                // Tab in the open popup when there is no remaining focused control.
                if state.open {
                    cx.defer_in(window, |state, window, cx| {
                        // Run after the app's default-focus fallback. An inner popup's
                        // focus already belongs to this popup and must be preserved.
                        if state.open
                            && !state.focus_handle.contains_focused(window, cx)
                            && state.focus_handle.within_focused(window, cx)
                        {
                            state.focus_handle.focus(window);
                        }
                    });
                }
            }));
            state
        });

        state.update(cx, |state, _| {
            if let Some(tracked_focus_handle) = tracked_focus_handle {
                state.tracked_focus_handle = Some(tracked_focus_handle);
            }
            state.on_open_change = self.on_open_change.clone();
            if let Some(force_open) = force_open {
                state.open = force_open;
            }
        });

        let open = state.read(cx).open;
        let opacity = if self.transition_offset.is_some() {
            state.update(cx, |state, cx| {
                let now = cx.background_executor().now();
                state.transition.retarget(f32::from(open), now);
                let (value, animating) = state.transition.value(now);
                if animating {
                    window.request_animation_frame();
                }
                value
            })
        } else {
            f32::from(open)
        };
        let focus_handle = state.read(cx).focus_handle.clone();
        let trigger_bounds = state.read(cx).trigger_bounds;

        let Some(trigger) = self.trigger else {
            return div().id("empty");
        };

        let parent_view_id = window.current_view();

        let el = div()
            .id(self.id)
            .child((trigger)(open, state.clone(), window, cx))
            .on_mouse_down(self.mouse_button, {
                let state = state.clone();
                move |_, window, cx| {
                    state.update(cx, |state, cx| {
                        // We force set open to false to toggle it correctly.
                        // Because if the mouse down out will toggle open first.
                        state.open = open;
                        state.toggle_open(window, cx);
                    });
                    cx.notify(parent_view_id);
                }
            })
            .child(
                canvas(
                    {
                        let state = state.clone();
                        move |bounds, _, cx| {
                            state.update(cx, |state, _| {
                                state.trigger_bounds = Some(bounds);
                            })
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );

        if !open && opacity == 0. {
            return el;
        }

        let content = anchored()
            .snap_to_window_with_margin(self.viewport_margin)
            .anchor(self.anchor)
            .when_some(trigger_bounds, |this, trigger_bounds| {
                let mut position = Self::resolved_corner(self.anchor, trigger_bounds);
                match self.center_axis {
                    Some(Axis::Horizontal) => position.x = trigger_bounds.center().x,
                    Some(Axis::Vertical) => {
                        position.x = match self.anchor {
                            Corner::TopRight | Corner::BottomRight => trigger_bounds.left(),
                            _ => trigger_bounds.right(),
                        };
                        position.y = trigger_bounds.center().y - trigger_bounds.size.height;
                    }
                    None => {}
                }
                this.position(position)
            })
            .when_some(self.center_axis, |this, axis| this.center_on(axis))
            .when_some(self.offset, |this, offset| {
                let (x, y) = if self.center_axis == Some(Axis::Vertical) {
                    (
                        match self.anchor {
                            Corner::TopRight | Corner::BottomRight => -offset,
                            _ => offset,
                        },
                        px(0.),
                    )
                } else {
                    (
                        px(0.),
                        match self.anchor {
                            Corner::BottomRight | Corner::BottomLeft => -offset,
                            _ => offset,
                        },
                    )
                };
                this.offset(Point { x, y })
            })
            .when(self.center_axis.is_some(), |this| {
                if let Some(mut target) = trigger_bounds {
                    target.origin.y -= target.size.height;
                    this.floating_on(target, self.offset.unwrap_or(px(4.)), self.viewport_margin)
                } else {
                    this
                }
            })
            .when_some(self.side, |this, anchor| {
                if let Some(mut target) = trigger_bounds {
                    target.origin.y -= target.size.height;
                    this.floating_side_on(
                        target,
                        self.offset.unwrap_or(px(4.)),
                        self.viewport_margin,
                        anchor,
                    )
                } else {
                    this
                }
            })
            .child(
                v_flex()
                    .id("content")
                    .track_focus(&focus_handle)
                    .key_context(CONTEXT)
                    .on_action(window.listener_for(&state, PopoverState::on_action_cancel))
                    .size_full()
                    .relative()
                    .when_some(self.transition_offset, |this, offset| {
                        this.opacity(opacity)
                            .left(offset.x * (1. - opacity))
                            .top(offset.y * (1. - opacity))
                    })
                    .occlude()
                    .tab_group()
                    .when(self.appearance, |this| this.popover_style(cx).p_3())
                    .when(self.offset.is_none(), |this| match self.anchor {
                        Corner::TopLeft | Corner::TopRight => this.top_1(),
                        Corner::BottomLeft | Corner::BottomRight => this.bottom_1(),
                    })
                    .when_some(self.content, |this, content| {
                        this.child(state.update(cx, |state, cx| (content)(state, window, cx)))
                    })
                    .children(self.children)
                    .child(
                        canvas(
                            {
                                let state = state.clone();
                                move |bounds, _, cx| {
                                    state.update(cx, |state, _| state.content_bounds = Some(bounds))
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .inset_0()
                        .size_full(),
                    )
                    .when(self.overlay_closable, |this| {
                        this.on_mouse_down_out({
                            let state = state.clone();
                            move |_, window, cx| {
                                let focus = state.read(cx).focus_handle.clone();
                                let inside_child = cx
                                    .global::<OpenPopovers>()
                                    .0
                                    .iter()
                                    .filter_map(|state| state.upgrade())
                                    .any(|child| {
                                        let child = child.read(cx);
                                        child.open
                                            && child.content_bounds.is_some_and(|bounds| {
                                                bounds.contains(&window.mouse_position())
                                            })
                                            && focus.contains(&child.focus_handle, window)
                                    });
                                if inside_child {
                                    return;
                                }
                                state.update(cx, |state, cx| {
                                    state.dismiss(window, cx);
                                });
                                cx.notify(parent_view_id);
                            }
                        })
                    })
                    .refine_style(&self.style),
            );
        el.child(if self.defer_content {
            deferred(content).with_priority(1).into_any_element()
        } else {
            content.into_any_element()
        })
    }
}
