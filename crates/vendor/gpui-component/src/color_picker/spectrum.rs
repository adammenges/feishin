// Hue/SV conversion and gestures follow Mantine 9.3.1 (bundled MIT license).
use super::*;
use gpui::{
    AccessibleAction, AnyElement, DispatchPhase, Rgba, hsla, linear_color_stop, linear_gradient,
    rems,
};

pub(super) fn from_color(color: Hsla) -> [f64; 3] {
    let rgba: Rgba = color.into();
    let [r, g, b] = [rgba.r, rgba.g, rgba.b].map(|value| (value as f64 * 255.).round());
    let max = r.max(g).max(b);
    let delta = max - r.min(g).min(b);
    let hue = if delta == 0. {
        0.
    } else if max == r {
        (g - b) / delta
    } else if max == g {
        2. + (b - r) / delta
    } else {
        4. + (r - g) / delta
    };
    [
        60. * if hue < 0. { hue + 6. } else { hue },
        if max == 0. { 0. } else { delta / max * 100. },
        max / 255. * 100.,
    ]
    .map(|value| (value * 1000.).round() / 1000.)
}

fn to_color([h, s, v]: [f64; 3]) -> Hsla {
    // Preserve the original order: h / 60 changes byte rounding at 170 degrees.
    let h = h / 360. * 6.;
    let sector = h.floor() as usize % 6;
    let (s, v) = (s / 100., v / 100.);
    let low = v * (1. - s);
    let falling = v * (1. - (h - h.floor()) * s);
    let rising = v * (1. - (1. - h + h.floor()) * s);
    let [r, g, b] = [
        [v, falling, low, low, rising, v][sector],
        [rising, v, v, falling, low, low][sector],
        [low, low, rising, v, v, falling][sector],
    ]
    .map(|value| (value * 255.).round() as f32 / 255.);
    Rgba { r, g, b, a: 1. }.into()
}

impl ColorPickerState {
    /// Set and commit an opaque color using hue, saturation and brightness.
    pub fn set_hsv(&mut self, hsv: [f64; 3], window: &mut Window, cx: &mut Context<Self>) {
        if !hsv.iter().all(|value| value.is_finite()) {
            return;
        }
        let hsv = [
            hsv[0].clamp(0., 360.),
            hsv[1].clamp(0., 100.),
            hsv[2].clamp(0., 100.),
        ];
        self.update_value(Some(to_color(hsv)), true, window, cx);
        // Keep the selected hue at black/gray and avoid RGB quantization moving the thumb.
        self.hsv = hsv;
    }

    fn spectrum_pointer(&mut self, field: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        let bounds = self.spectrum_bounds[field];
        let x = ((position.x - bounds.left()) / bounds.size.width.max(px(1.))).clamp(0., 1.) as f64;
        let y = ((position.y - bounds.top()) / bounds.size.height.max(px(1.))).clamp(0., 1.) as f64;
        if field == 0 {
            self.hsv[1] = (x * 100.).round();
            self.hsv[2] = ((1. - y) * 100.).round();
        } else {
            self.hsv[0] = (x * 360.).round();
        }
        self.hovered_color = Some(to_color(self.hsv));
        cx.emit(ColorPickerEvent::Preview(self.hovered_color));
        cx.notify();
    }

    fn spectrum_key(
        &mut self,
        field: usize,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut hsv = self.hsv;
        match (field, key) {
            (0, "left") => hsv[1] -= 5.,
            (0, "right") => hsv[1] += 5.,
            (0, "up") => hsv[2] += 5.,
            (0, "down") => hsv[2] -= 5.,
            (1, "left") => hsv[0] -= 18.,
            (1, "right") => hsv[0] += 18.,
            _ => return false,
        }
        if field == 0 {
            hsv[1] = hsv[1].round();
            hsv[2] = hsv[2].round();
        } else {
            hsv[0] = hsv[0].round();
        }
        self.set_hsv(hsv, window, cx);
        true
    }
}

impl ColorPicker {
    pub(super) fn render_spectrum(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        let state = self.state.read(cx);
        let hsv = state.hsv;
        let preview = to_color(hsv);
        let rgba: Rgba = preview.into();
        let text = format!(
            "rgb({}, {}, {})",
            (rgba.r * 255.).round() as u8,
            (rgba.g * 255.).round() as u8,
            (rgba.b * 255.).round() as u8
        );
        let focus = state.spectrum_focus.clone();
        let controls = (0..2)
            .map(|field| {
                let x = if field == 0 {
                    hsv[1] / 100.
                } else {
                    hsv[0] / 360.
                };
                let y = if field == 0 { 1. - hsv[2] / 100. } else { 0.5 };
                let canvas_state = self.state.clone();
                let pointer = self.state.clone();
                div()
                    .id(if field == 0 {
                        "color-saturation"
                    } else {
                        "color-hue"
                    })
                    .debug_selector(move || {
                        if field == 0 {
                            "color-saturation".into()
                        } else {
                            "color-hue".into()
                        }
                    })
                    .relative()
                    .mx(rems(0.375))
                    .h(rems(if field == 0 { 6.875 } else { 0.875 }))
                    .when(field == 0, |div| div.my(rems(0.375)))
                    .when(field == 1, |div| div.mt(rems(0.25)))
                    .role(gpui::Role::Slider)
                    .aria_label(if field == 0 {
                        "Color saturation"
                    } else {
                        "Hue"
                    })
                    .aria_value(text.clone())
                    .aria_numeric_value(if field == 0 { x } else { hsv[0] })
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(if field == 0 { 1. } else { 360. })
                    .track_focus(&focus[field].clone().tab_stop(true))
                    .key_context("FormControl")
                    .on_key_down(window.listener_for(
                        &self.state,
                        move |state, event: &gpui::KeyDownEvent, window, cx| {
                            if state.spectrum_key(field, &event.keystroke.key, window, cx) {
                                window.prevent_default();
                                cx.stop_propagation();
                            }
                        },
                    ))
                    .on_a11y_action(
                        AccessibleAction::SetValue,
                        window.listener_for(&self.state, move |state, data, window, cx| {
                            if let Some(gpui::accesskit::ActionData::NumericValue(value)) = data {
                                let mut hsv = state.hsv;
                                if field == 0 {
                                    hsv[1] = (value * 100.).round();
                                } else {
                                    hsv[0] = value.round();
                                }
                                state.set_hsv(hsv, window, cx);
                            }
                        }),
                    )
                    .on_a11y_action(
                        AccessibleAction::Increment,
                        window.listener_for(&self.state, move |state, _, window, cx| {
                            state.spectrum_key(field, "right", window, cx);
                        }),
                    )
                    .on_a11y_action(
                        AccessibleAction::Decrement,
                        window.listener_for(&self.state, move |state, _, window, cx| {
                            state.spectrum_key(field, "left", window, cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        window.listener_for(
                            &self.state,
                            move |state, event: &gpui::MouseDownEvent, window, cx| {
                                state.spectrum_focus[field].focus(window);
                                state.spectrum_drag = Some(field);
                                state.spectrum_pointer(field, event.position, cx);
                                cx.stop_propagation();
                            },
                        ),
                    )
                    .child(if field == 0 {
                        div()
                            .absolute()
                            .inset(rems(-0.4375))
                            .rounded(rems(0.1875))
                            .bg(hsla((hsv[0] / 360.) as f32, 1., 0.5, 1.))
                            .child(div().absolute().inset_0().rounded(rems(0.1875)).bg(
                                linear_gradient(
                                    90.,
                                    linear_color_stop(gpui::white(), 0.),
                                    // Straight RGBA interpolation must retain white at zero alpha.
                                    linear_color_stop(hsla(0., 0., 1., 0.), 1.),
                                ),
                            ))
                            .child(div().absolute().inset_0().rounded(rems(0.1875)).bg(
                                linear_gradient(
                                    0.,
                                    linear_color_stop(gpui::black(), 0.),
                                    linear_color_stop(gpui::transparent_black(), 1.),
                                ),
                            ))
                    } else {
                        let colors = [0., 60., 120., 170., 240., 300., 360.];
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(rems(-0.4375))
                            .right(rems(-0.4375))
                            .h_flex()
                            .overflow_hidden()
                            .rounded_full()
                            .children(
                                colors
                                    .windows(2)
                                    .map(|pair| {
                                        div().flex_1().h_full().bg(linear_gradient(
                                            90.,
                                            linear_color_stop(
                                                hsla(pair[0] / 360., 1., 0.5, 1.),
                                                0.,
                                            ),
                                            linear_color_stop(
                                                hsla(pair[1] / 360., 1., 0.5, 1.),
                                                1.,
                                            ),
                                        ))
                                    })
                                    .collect::<Vec<_>>(),
                            )
                    })
                    .child(
                        div()
                            .debug_selector(move || {
                                if field == 0 {
                                    "color-saturation-thumb".into()
                                } else {
                                    "color-hue-thumb".into()
                                }
                            })
                            .absolute()
                            .left(relative(x as f32))
                            .top(relative(y as f32))
                            .ml(rems(-0.375))
                            .mt(rems(-0.375))
                            .size(rems(0.75))
                            .rounded_full()
                            .border_2()
                            .border_color(gpui::white())
                            .bg(if field == 0 {
                                preview
                            } else {
                                hsla((hsv[0] / 360.) as f32, 1., 0.5, 1.)
                            })
                            .shadow_xs(),
                    )
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                canvas_state
                                    .update(cx, |state, _| state.spectrum_bounds[field] = bounds);
                            },
                            move |_, _, window, _| {
                                if field != 0 {
                                    return;
                                }
                                let release = pointer.clone();
                                window.on_mouse_event(
                                    move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                                        if phase == DispatchPhase::Bubble
                                            && event.pressed_button == Some(MouseButton::Left)
                                            && let Some(field) = pointer.read(cx).spectrum_drag
                                        {
                                            pointer.update(cx, |state, cx| {
                                                state.spectrum_pointer(field, event.position, cx)
                                            });
                                        }
                                    },
                                );
                                window.on_mouse_event(
                                    move |event: &gpui::MouseUpEvent, phase, window, cx| {
                                        if phase == DispatchPhase::Capture
                                            && event.button == MouseButton::Left
                                            && release.read(cx).spectrum_drag.is_some()
                                        {
                                            release.update(cx, |state, cx| {
                                                state.spectrum_drag = None;
                                                state.set_hsv(state.hsv, window, cx);
                                            });
                                        }
                                    },
                                );
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
            })
            .collect::<Vec<_>>();
        v_flex()
            .debug_selector(|| "color-spectrum".into())
            .w(rems(12.5))
            .p(rems(0.0625))
            .children(controls)
            .child(
                h_flex()
                    .mt(rems(0.4375))
                    .pb(rems(0.125))
                    .gap(rems(0.25))
                    .children(
                        self.featured_colors
                            .clone()
                            .unwrap_or_default()
                            .iter()
                            .map(|color| {
                                self.render_item(*color, true, window, cx)
                                    .flex_1()
                                    .h(window.rem_size() * 2.275 + px(1.))
                                    .rounded_full()
                            })
                            .collect::<Vec<_>>(),
                    ),
            )
            .into_any_element()
    }
}
