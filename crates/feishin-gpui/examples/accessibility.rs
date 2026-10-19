//! Native AT-SPI fixture for an isolated Linux display.
#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() {
    use gpui::{prelude::*, *};
    use gpui_component::{
        Disableable, Root, StyledExt,
        button::Button,
        checkbox::Checkbox,
        input::{Input, InputState},
        slider::{Slider, SliderEvent, SliderState},
    };
    use std::time::Duration;
    struct Controls {
        checked: bool,
        input: Entity<InputState>,
        password: Entity<InputState>,
        slider: Entity<SliderState>,
    }
    impl Render for Controls {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("fixture")
                .role(Role::Application)
                .aria_label("Feishin accessibility fixture")
                .v_flex()
                .size_full()
                .p_4()
                .gap_3()
                .child(
                    Checkbox::new("shuffle")
                        .label("Shuffle")
                        .checked(self.checked)
                        .on_click(cx.listener(|this, checked, _, cx| {
                            this.checked = *checked;
                            eprintln!("FEISHIN_ATSPI_SHUFFLE {}", this.checked);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("disabled")
                        .label("Disabled action")
                        .disabled(true)
                        .on_click(|_, _, _| panic!("Disabled accessibility action executed")),
                )
                .child(Input::new(&self.input))
                .child(Input::new(&self.password).mask_toggle())
                .child(Slider::new(&self.slider).aria_label("Volume"))
                .child(
                    div()
                        .id("status")
                        .role(Role::Status)
                        .aria_live(accesskit::Live::Polite)
                        .aria_label(if self.checked {
                            "Shuffle enabled"
                        } else {
                            "Ready"
                        })
                        .child(if self.checked {
                            "Shuffle enabled"
                        } else {
                            "Ready"
                        }),
                )
                .child(div().hidden().child("Hidden secret"))
        }
    }
    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .run(|cx| {
            gpui_component::init(cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(600.), px(500.)),
                    ))),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("Feishin AT-SPI fixture");
                    let view = cx.new(|cx| {
                        let input =
                            cx.new(|cx| InputState::new(window, cx).placeholder("Username"));
                        let password = cx.new(|cx| {
                            InputState::new(window, cx)
                                .placeholder("Password")
                                .masked(true)
                                .default_value("private-atspi-secret")
                        });
                        let slider = cx.new(|_| {
                            SliderState::new()
                                .min(0.)
                                .max(100.)
                                .step(1.)
                                .default_value(50.)
                        });
                        cx.subscribe(&slider, |_, _, event: &SliderEvent, cx| {
                            let SliderEvent::Change(value) = event else {
                                return;
                            };
                            eprintln!("FEISHIN_ATSPI_VOLUME {}", value.end());
                            cx.notify();
                        })
                        .detach();
                        Controls {
                            checked: false,
                            input,
                            password,
                            slider,
                        }
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .unwrap();
            cx.spawn(async move |cx| {
                smol::Timer::after(Duration::from_secs(40)).await;
                cx.update(|cx| cx.quit()).unwrap();
            })
            .detach();
            cx.activate(true);
        });
}
