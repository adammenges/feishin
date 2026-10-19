use std::{rc::Rc, time::Duration};

use gpui::{
    AbsoluteLength, Animation, AnimationExt as _, AnyElement, App, Bounds, BoxShadow, ClickEvent,
    Div, Edges, FocusHandle, Hsla, InteractiveElement, IntoElement, KeyBinding, MouseButton,
    ParentElement, Pixels, Point, RenderOnce, SharedString, StatefulInteractiveElement,
    StyleRefinement, Styled, Window, anchored, div, hsla, point, prelude::FluentBuilder, px,
    relative, rems,
};
use rust_i18n::t;

use crate::{
    ActiveTheme as _, IconName, Sizable as _, StyledExt, WindowExt as _,
    actions::{Cancel, Confirm},
    animation::cubic_bezier,
    button::{Button, ButtonVariant, ButtonVariants as _},
    h_flex,
    scroll::ScrollableElement as _,
    v_flex,
};

const CONTEXT: &str = "Dialog";
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, Some(CONTEXT)),
        KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
    ]);
}

type RenderButtonFn = Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement>;
type FooterFn =
    Box<dyn Fn(RenderButtonFn, RenderButtonFn, &mut Window, &mut App) -> Vec<AnyElement>>;

/// Application styles for the shared dialog surfaces and their descendants.
#[derive(Clone)]
pub struct DialogStyles {
    pub overlay: crate::StateStyle,
    pub content: crate::StateStyle,
    pub header: crate::StateStyle,
    pub title: crate::StateStyle,
    pub body: crate::StateStyle,
    pub close: crate::StateStyle,
    pub close_icon: Rc<dyn Fn(&App) -> crate::Icon>,
    pub close_label: Rc<dyn Fn(&App) -> SharedString>,
    pub wrap_body: Rc<dyn Fn(AnyElement) -> AnyElement>,
}
impl gpui::Global for DialogStyles {}

/// Dialog button props.
pub struct DialogButtonProps {
    ok_text: Option<SharedString>,
    ok_variant: ButtonVariant,
    cancel_text: Option<SharedString>,
    cancel_variant: ButtonVariant,
}

impl Default for DialogButtonProps {
    fn default() -> Self {
        Self {
            ok_text: None,
            ok_variant: ButtonVariant::Primary,
            cancel_text: None,
            cancel_variant: ButtonVariant::default(),
        }
    }
}

impl DialogButtonProps {
    /// Sets the text of the OK button. Default is `OK`.
    pub fn ok_text(mut self, ok_text: impl Into<SharedString>) -> Self {
        self.ok_text = Some(ok_text.into());
        self
    }

    /// Sets the variant of the OK button. Default is `ButtonVariant::Primary`.
    pub fn ok_variant(mut self, ok_variant: ButtonVariant) -> Self {
        self.ok_variant = ok_variant;
        self
    }

    /// Sets the text of the Cancel button. Default is `Cancel`.
    pub fn cancel_text(mut self, cancel_text: impl Into<SharedString>) -> Self {
        self.cancel_text = Some(cancel_text.into());
        self
    }

    /// Sets the variant of the Cancel button. Default is `ButtonVariant::default()`.
    pub fn cancel_variant(mut self, cancel_variant: ButtonVariant) -> Self {
        self.cancel_variant = cancel_variant;
        self
    }
}

/// A modal to display content in a dialog box.
#[derive(IntoElement)]
pub struct Dialog {
    style: StyleRefinement,
    title: Option<AnyElement>,
    accessible_label: Option<SharedString>,
    footer: Option<FooterFn>,
    content: Div,
    width: AbsoluteLength,
    max_width: Option<Pixels>,
    body_height: Option<Pixels>,
    body_scroll: Option<gpui::ScrollHandle>,
    margin_top: Option<Pixels>,

    on_close: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
    on_ok: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static>>,
    on_cancel: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static>,
    button_props: DialogButtonProps,
    close_button: bool,
    overlay: bool,
    overlay_closable: bool,
    keyboard: bool,
    pop_distance: Option<Pixels>,
    pub(crate) fade_only: bool,
    pub(crate) fade_opacity: Option<f32>,
    pub(crate) closing: bool,
    pub(crate) top_layer: bool,
    pub(crate) id: SharedString,

    /// This will be change when open the dialog, the focus handle is create when open the dialog.
    pub(crate) focus_handle: FocusHandle,
    pub(crate) layer_ix: usize,
    pub(crate) overlay_visible: bool,
}

pub(crate) fn overlay_color(overlay: bool, cx: &App) -> Hsla {
    if !overlay {
        return hsla(0., 0., 0., 0.);
    }

    cx.theme().overlay
}

impl Dialog {
    /// Create a new dialog.
    pub fn new(_: &mut Window, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            style: StyleRefinement::default(),
            title: None,
            accessible_label: None,
            footer: None,
            content: v_flex(),
            margin_top: None,
            width: rems(27.5).into(),
            max_width: None,
            body_height: None,
            body_scroll: None,
            overlay: true,
            keyboard: true,
            pop_distance: None,
            fade_only: true,
            fade_opacity: None,
            closing: false,
            top_layer: true,
            id: "dialog".into(),
            layer_ix: 0,
            overlay_visible: false,
            on_close: Rc::new(|_, _, _| {}),
            on_ok: None,
            on_cancel: Rc::new(|_, _, _| true),
            button_props: DialogButtonProps::default(),
            close_button: true,
            overlay_closable: true,
        }
    }

    /// Sets the title of the dialog.
    pub fn title(mut self, title: impl IntoElement) -> Self {
        self.title = Some(title.into_any_element());
        self
    }

    /// Name the dialog for assistive technology, including dialogs without a heading.
    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    /// Set the footer of the dialog.
    ///
    /// The `footer` is a function that takes two `RenderButtonFn` and a `WindowContext` and returns a list of `AnyElement`.
    ///
    /// - First `RenderButtonFn` is the render function for the OK button.
    /// - Second `RenderButtonFn` is the render function for the CANCEL button.
    ///
    /// When you set the footer, the footer will be placed default footer buttons.
    pub fn footer<E, F>(mut self, footer: F) -> Self
    where
        E: IntoElement,
        F: Fn(RenderButtonFn, RenderButtonFn, &mut Window, &mut App) -> Vec<E> + 'static,
    {
        self.footer = Some(Box::new(move |ok, cancel, window, cx| {
            footer(ok, cancel, window, cx)
                .into_iter()
                .map(|e| e.into_any_element())
                .collect()
        }));
        self
    }

    /// Set to use confirm dialog, with OK and Cancel buttons.
    ///
    /// See also [`Self::alert`]
    pub fn confirm(self) -> Self {
        self.footer(|ok, cancel, window, cx| vec![cancel(window, cx), ok(window, cx)])
            .overlay_closable(false)
            .close_button(false)
    }

    /// Set to as a alter dialog, with OK button.
    ///
    /// See also [`Self::confirm`]
    pub fn alert(self) -> Self {
        self.footer(|ok, _, window, cx| vec![ok(window, cx)])
            .overlay_closable(false)
            .close_button(false)
    }

    /// Set the button props of the dialog.
    pub fn button_props(mut self, button_props: DialogButtonProps) -> Self {
        self.button_props = button_props;
        self
    }

    /// Sets the callback for when the dialog is closed.
    ///
    /// Called after [`Self::on_ok`] or [`Self::on_cancel`] callback.
    pub fn on_close(
        mut self,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Rc::new(on_close);
        self
    }

    /// Sets the callback for when the dialog is has been confirmed.
    ///
    /// The callback should return `true` to close the dialog, if return `false` the dialog will not be closed.
    pub fn on_ok(
        mut self,
        on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.on_ok = Some(Rc::new(on_ok));
        self
    }

    /// Sets the callback for when the dialog is has been canceled.
    ///
    /// The callback should return `true` to close the dialog, if return `false` the dialog will not be closed.
    pub fn on_cancel(
        mut self,
        on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.on_cancel = Rc::new(on_cancel);
        self
    }

    /// Sets the false to hide close icon, default: true
    pub fn close_button(mut self, close_button: bool) -> Self {
        self.close_button = close_button;
        self
    }

    /// Set an explicit top offset; otherwise the dialog is centered.
    pub fn margin_top(mut self, margin_top: Pixels) -> Self {
        self.margin_top = Some(margin_top);
        self
    }

    /// Sets the width of the dialog, defaults to 27.5rem.
    ///
    /// See also [`Self::width`]
    pub fn w(mut self, width: impl Into<AbsoluteLength>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the width of the dialog, defaults to 27.5rem.
    pub fn width(mut self, width: impl Into<AbsoluteLength>) -> Self {
        self.width = width.into();
        self
    }

    /// Set the maximum width of the dialog, defaults to `None`.
    pub fn max_w(mut self, max_width: Pixels) -> Self {
        self.max_width = Some(max_width);
        self
    }

    /// Share the body's scroll handle with content that navigates by keyboard.
    pub fn body_scroll_handle(mut self, handle: gpui::ScrollHandle) -> Self {
        self.body_scroll = Some(handle);
        self
    }

    /// Set the body's natural height while keeping its viewport scrollable.
    pub fn body_height(mut self, height: Pixels) -> Self {
        self.body_height = Some(height);
        self
    }

    /// Set the overlay of the dialog, defaults to `true`.
    pub fn overlay(mut self, overlay: bool) -> Self {
        self.overlay = overlay;
        self
    }

    /// Set the overlay closable of the dialog, defaults to `true`.
    ///
    /// When the overlay is clicked, the dialog will be closed.
    pub fn overlay_closable(mut self, overlay_closable: bool) -> Self {
        self.overlay_closable = overlay_closable;
        self
    }

    /// Set whether to support keyboard esc to close the dialog, defaults to `true`.
    pub fn keyboard(mut self, keyboard: bool) -> Self {
        self.keyboard = keyboard;
        self
    }

    /// Use a 300 ms opacity transition without translating the dialog.
    pub fn fade_only(mut self, fade_only: bool) -> Self {
        self.fade_only = fade_only;
        self
    }

    /// Use the original 300 ms pop transition, including its retained exit.
    pub fn pop_transition(mut self, distance: Pixels) -> Self {
        self.pop_distance = Some(distance);
        self.fade_only = true;
        self
    }

    pub(crate) fn has_overlay(&self) -> bool {
        self.overlay
    }
}

impl ParentElement for Dialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl Styled for Dialog {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let styles = cx.try_global::<DialogStyles>().cloned();
        let resolve = |select: fn(&DialogStyles) -> &crate::StateStyle| {
            styles
                .as_ref()
                .map(|styles| select(styles)(&[], cx))
                .unwrap_or_default()
        };
        let overlay_style = resolve(|styles| &styles.overlay);
        let content_style = resolve(|styles| &styles.content);
        let header_style = resolve(|styles| &styles.header);
        let title_style = resolve(|styles| &styles.title);
        let body_style = resolve(|styles| &styles.body);
        let custom_shadow = self
            .style
            .box_shadow
            .clone()
            .or_else(|| content_style.box_shadow.clone());
        let layer_ix = self.layer_ix;
        let on_close = self.on_close.clone();
        let on_ok = self.on_ok.clone();
        let on_cancel = self.on_cancel.clone();

        let render_ok: RenderButtonFn = Box::new({
            let on_ok = on_ok.clone();
            let on_close = on_close.clone();
            let ok_text = self
                .button_props
                .ok_text
                .unwrap_or_else(|| t!("Dialog.ok").into());
            let ok_variant = self.button_props.ok_variant;
            move |_, _| {
                Button::new("ok")
                    .label(ok_text)
                    .with_variant(ok_variant)
                    .on_click({
                        let on_ok = on_ok.clone();
                        let on_close = on_close.clone();

                        move |_, window, cx| {
                            if let Some(on_ok) = &on_ok {
                                if !on_ok(&ClickEvent::default(), window, cx) {
                                    return;
                                }
                            }

                            on_close(&ClickEvent::default(), window, cx);
                            window.close_dialog(cx);
                        }
                    })
                    .into_any_element()
            }
        });
        let render_cancel: RenderButtonFn = Box::new({
            let on_cancel = on_cancel.clone();
            let on_close = on_close.clone();
            let cancel_text = self
                .button_props
                .cancel_text
                .unwrap_or_else(|| t!("Dialog.cancel").into());
            let cancel_variant = self.button_props.cancel_variant;
            move |_, _| {
                Button::new("cancel")
                    .label(cancel_text)
                    .with_variant(cancel_variant)
                    .on_click({
                        let on_cancel = on_cancel.clone();
                        let on_close = on_close.clone();
                        move |_, window, cx| {
                            if !on_cancel(&ClickEvent::default(), window, cx) {
                                return;
                            }

                            on_close(&ClickEvent::default(), window, cx);
                            window.close_dialog(cx);
                        }
                    })
                    .into_any_element()
            }
        });

        let window_paddings = crate::window_border::window_paddings(window);
        let view_size = window.viewport_size()
            - gpui::size(
                window_paddings.left + window_paddings.right,
                window_paddings.top + window_paddings.bottom,
            );
        let bounds = Bounds {
            origin: Point::default(),
            size: view_size,
        };
        let rem_size = window.rem_size();
        let width = self.width.to_pixels(rem_size);
        // Explicit offsets preserve the artwork viewer's fullscreen frame.
        let centered = self.margin_top.is_none();
        let rtl = window.text_style().text_direction == gpui::TextDirection::Rtl;
        let offset_top = px(layer_ix as f32 * 16.);
        let y = self.margin_top.unwrap_or(view_size.height / 10.) + offset_top;
        let x = bounds.center().x - width / 2.;

        let base_size = window.text_style().font_size;

        let mut paddings = Edges::all(if centered { rem_size * 0.75 } else { px(24.) });
        if let Some(pl) = self.style.padding.left {
            paddings.left = pl.to_pixels(base_size, rem_size);
        }
        if let Some(pr) = self.style.padding.right {
            paddings.right = pr.to_pixels(base_size, rem_size);
        }
        if let Some(pt) = self.style.padding.top {
            paddings.top = pt.to_pixels(base_size, rem_size);
        }
        if let Some(pb) = self.style.padding.bottom {
            paddings.bottom = pb.to_pixels(base_size, rem_size);
        }

        let fade_only = self.fade_only;
        let fade_opacity = self.fade_opacity;
        let closing = self.closing;
        let pop_distance = self.pop_distance;
        let animation = Animation::new(Duration::from_secs_f64(if fade_only { 0.3 } else { 0.25 }))
            .with_easing(if fade_only {
                cubic_bezier(0.25, 0.1, 0.25, 1.)
            } else {
                cubic_bezier(0.32, 0.72, 0., 1.)
            });

        anchored()
            .position(point(window_paddings.left, window_paddings.top))
            .snap_to_window()
            .child(
                div()
                    .id(self.id.clone())
                    .debug_selector(move || format!("native-modal-overlay-{layer_ix}"))
                    .occlude()
                    .w(view_size.width)
                    .h(view_size.height)
                    .when(centered, |this| this.flex().items_center().justify_center())
                    .when(self.overlay_visible, |this| {
                        this.bg(overlay_color(self.overlay, cx))
                            .refine_style(&overlay_style)
                    })
                    .map(|this| {
                        // Only the last dialog owns the `mouse down - close dialog` event.
                        if !self.top_layer || closing {
                            return this;
                        }

                        this.on_any_mouse_down({
                            let on_cancel = on_cancel.clone();
                            let on_close = on_close.clone();
                            move |event, window, cx| {
                                cx.stop_propagation();

                                if self.overlay_closable && event.button == MouseButton::Left {
                                    if !on_cancel(&ClickEvent::default(), window, cx) {
                                        return;
                                    }
                                    on_close(&ClickEvent::default(), window, cx);
                                    window.close_dialog(cx);
                                }
                            }
                        })
                    })
                    .child(
                        v_flex()
                            .id(layer_ix)
                            .role(gpui::accesskit::Role::Dialog)
                            .when_some(self.accessible_label, |this, label| this.aria_label(label))
                            .debug_selector(move || format!("native-modal-content-{layer_ix}"))
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius_lg)
                            .min_h_24()
                            .pt(paddings.top)
                            .pb(paddings.bottom)
                            .gap(paddings.top.min(px(16.)))
                            .when(centered, |this| {
                                this.min_h_0()
                                    .pt_0()
                                    .pb_0()
                                    .gap_0()
                                    .text_size(rem_size * 0.875)
                                    .line_height(rem_size * 1.125)
                                    .overflow_hidden()
                            })
                            .px_0()
                            .key_context(CONTEXT)
                            .track_focus(&self.focus_handle)
                            .tab_group()
                            .when(self.keyboard && !closing, |this| {
                                this.on_action({
                                    let on_cancel = on_cancel.clone();
                                    let on_close = on_close.clone();
                                    move |_: &Cancel, window, cx| {
                                        if !on_cancel(&ClickEvent::default(), window, cx) {
                                            return;
                                        }
                                        on_close(&ClickEvent::default(), window, cx);
                                        window.close_dialog(cx);
                                    }
                                })
                                .on_action({
                                    let on_ok = on_ok.clone();
                                    let on_close = on_close.clone();
                                    let has_footer = self.footer.is_some();
                                    move |_: &Confirm, window, cx| {
                                        if let Some(on_ok) = &on_ok {
                                            if on_ok(&ClickEvent::default(), window, cx) {
                                                on_close(&ClickEvent::default(), window, cx);
                                                window.close_dialog(cx);
                                            }
                                        } else if has_footer {
                                            window.close_dialog(cx);
                                        }
                                    }
                                })
                            })
                            // There style is high priority, can't be overridden.
                            .occlude()
                            .relative()
                            .when(!centered, |this| this.absolute().left(x).top(y))
                            .w(width)
                            .flex_shrink_0()
                            .when(centered, |this| {
                                this.max_w(view_size.width * 0.9)
                                    .max_h(view_size.height * 0.9)
                            })
                            .when_some(self.max_width, |this, w| this.max_w(w))
                            .refine_style(&content_style)
                            .refine_style(&self.style)
                            .when(!centered, |this| this.px_0())
                            .when(
                                self.title.is_some() || (centered && self.close_button),
                                |this| {
                                    this.child(
                                        div()
                                            .debug_selector(move || {
                                                format!("native-modal-header-{layer_ix}")
                                            })
                                            .pl(paddings.left)
                                            .pr(paddings.right)
                                            .line_height(relative(1.))
                                            .font_semibold()
                                            .when(centered, |this| {
                                                this.flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .flex_shrink_0()
                                                    .min_h(rem_size * 3.75)
                                                    .py(paddings.top)
                                                    .pl(if rtl {
                                                        paddings.left - rem_size * 0.3125
                                                    } else {
                                                        paddings.left
                                                    })
                                                    .pr(if rtl {
                                                        paddings.right
                                                    } else {
                                                        paddings.right - rem_size * 0.3125
                                                    })
                                            })
                                            .refine_style(&header_style)
                                            .when_some(self.title, |this, title| {
                                                this.child(
                                                    div()
                                                        .debug_selector(move || {
                                                            format!("native-modal-title-{layer_ix}")
                                                        })
                                                        .when(centered, |this| {
                                                            this.w_full()
                                                                .text_size(rem_size * 1.25)
                                                                .line_height(rem_size * 1.25)
                                                                .font_bold()
                                                        })
                                                        .refine_style(&title_style)
                                                        .child(title),
                                                )
                                            }),
                                    )
                                },
                            )
                            .children(self.close_button.then(|| {
                                let top = if centered {
                                    rem_size
                                } else {
                                    (paddings.top - px(10.)).max(px(8.))
                                };
                                let right = if centered {
                                    paddings.right
                                } else {
                                    (paddings.right - px(10.)).max(px(8.))
                                };

                                Button::new("close")
                                    .debug_selector(move || {
                                        format!("native-modal-close-{layer_ix}")
                                    })
                                    .absolute()
                                    .top(top)
                                    .right(right)
                                    .small()
                                    .ghost()
                                    .icon(IconName::Close)
                                    .when(centered, |button| {
                                        button
                                            .size(rem_size * 1.75)
                                            .p_0()
                                            .rounded(rem_size * 0.1875)
                                    })
                                    .when_some(styles.as_ref(), |button, styles| {
                                        button
                                            .state_style(styles.close.clone())
                                            .icon((styles.close_icon)(cx))
                                            .aria_label((styles.close_label)(cx))
                                    })
                                    .on_click({
                                        let on_cancel = self.on_cancel.clone();
                                        let on_close = self.on_close.clone();
                                        move |_, window, cx| {
                                            if !on_cancel(&ClickEvent::default(), window, cx) {
                                                return;
                                            }
                                            on_close(&ClickEvent::default(), window, cx);
                                            window.close_dialog(cx);
                                        }
                                    })
                            }))
                            .child(
                                div()
                                    .w_full()
                                    .min_h_0()
                                    .overflow_hidden()
                                    .debug_selector(move || format!("native-modal-body-{layer_ix}"))
                                    .child(
                                        v_flex()
                                            .id("contents")
                                            .pl(paddings.left)
                                            .pr(paddings.right)
                                            .when(centered, |this| this.py(paddings.top))
                                            .refine_style(&body_style)
                                            .overflow_y_scrollbar()
                                            .when_some(self.body_scroll, |this, handle| {
                                                this.scroll_handle(handle)
                                            })
                                            .child({
                                                let content = self
                                                    .content
                                                    .when_some(self.body_height, |body, height| {
                                                        body.h((height
                                                            - paddings.top
                                                            - paddings.bottom)
                                                            .max(px(0.)))
                                                            .flex_shrink_0()
                                                    })
                                                    .into_any_element();
                                                if let Some(styles) = &styles {
                                                    (styles.wrap_body)(content)
                                                } else {
                                                    content
                                                }
                                            }),
                                    ),
                            )
                            .when_some(self.footer, |this, footer| {
                                this.child(
                                    h_flex()
                                        .gap_2()
                                        .pl(paddings.left)
                                        .pr(paddings.right)
                                        .line_height(relative(1.))
                                        .justify_end()
                                        .children(footer(render_ok, render_cancel, window, cx)),
                                )
                            })
                            .map(|this| {
                                let render = move |this: gpui::Stateful<Div>, delta| {
                                    let y_offset = if fade_only { px(0.) } else { delta * px(30.) };
                                    // This is equivalent to `shadow_xl` with an extra opacity.
                                    let shadow = custom_shadow.clone().unwrap_or_else(|| {
                                        vec![
                                            BoxShadow {
                                                color: hsla(0., 0., 0., 0.1 * delta),
                                                offset: point(px(0.), px(20.)),
                                                blur_radius: px(25.),
                                                spread_radius: px(-5.),
                                            },
                                            BoxShadow {
                                                color: hsla(0., 0., 0., 0.1 * delta),
                                                offset: point(px(0.), px(8.)),
                                                blur_radius: px(10.),
                                                spread_radius: px(-6.),
                                            },
                                        ]
                                    });
                                    this.when(!centered, |this| this.top(y + y_offset))
                                        .when_some(pop_distance, |this, distance| {
                                            let scale = 0.9 + 0.1 * delta;
                                            this.paint_scale(scale)
                                                .top(distance * (1. - delta) * scale)
                                        })
                                        .shadow(shadow)
                                };
                                if let Some(opacity) = fade_opacity {
                                    render(this, opacity).into_any_element()
                                } else {
                                    this.with_animation("slide-down", animation.clone(), render)
                                        .into_any_element()
                                }
                            }),
                    )
                    .when(closing, |this| {
                        this.child(
                            div()
                                .absolute()
                                .inset_0()
                                .occlude()
                                .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                        )
                    })
                    .map(move |this| {
                        if let Some(opacity) = fade_opacity {
                            this.opacity(opacity).into_any_element()
                        } else {
                            this.with_animation("fade-in", animation, move |this, delta| {
                                this.opacity(delta)
                            })
                            .into_any_element()
                        }
                    }),
            )
    }
}
