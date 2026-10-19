use ::core::prelude::v1::test;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Root,
    slider::{SliderEvent, SliderState},
};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

struct Host {
    slider: Entity<SliderState>,
    vertical: bool,
    disabled: bool,
    extent: f32,
    outside: FocusHandle,
    events: Vec<(bool, f32)>,
    bubbled: usize,
    subscription: Option<Subscription>,
}
impl Host {
    fn configure(&mut self, case: &Value, cx: &mut Context<Self>) {
        self.vertical = case["vertical"].as_bool().unwrap_or(false);
        self.disabled = case["disabled"].as_bool().unwrap();
        self.extent = 200.;
        self.events.clear();
        self.bubbled = 0;
        cx.set_global(crate::appearance::Language(
            if case["rtl"] == true { "ar" } else { "en" }.into(),
        ));
        self.slider = cx.new(|_| {
            SliderState::new()
                .min(case["min"].as_f64().unwrap() as f32)
                .max(case["max"].as_f64().unwrap() as f32)
                .step(case["step"].as_f64().unwrap() as f32)
                .default_value(
                    case["value"]
                        .as_f64()
                        .unwrap_or(case["min"].as_f64().unwrap()) as f32,
                )
        });
        self.subscription = Some(
            cx.subscribe(&self.slider, |this, _, event: &SliderEvent, _| {
                this.events.push(match event {
                    SliderEvent::Change(value) => (false, value.end()),
                    SliderEvent::ChangeEnd(value) => (true, value.end()),
                });
            }),
        );
        cx.notify();
    }
}
impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_8()
            .track_focus(&self.outside)
            .on_key_down(cx.listener(|this, _, _, _| this.bubbled += 1))
            .child(
                div()
                    .id("slider-fixture")
                    .debug_selector(|| "slider-fixture".into())
                    .w(px(if self.vertical { 40. } else { self.extent }))
                    .h(px(if self.vertical { self.extent } else { 40. }))
                    .child(
                        crate::styles::slider(&self.slider, cx)
                            .aria_label("Test slider")
                            .disabled(self.disabled)
                            .when(self.vertical, |slider| slider.vertical().h_full()),
                    ),
            )
    }
}

#[gpui::test]
fn shared_slider_matches_original_keyboard_and_rtl_pointer_callbacks(cx: &mut TestAppContext) {
    use gpui::accesskit::{Action, ActionRequest, TreeId};
    cx.update(gpui_component::init);
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_accessibility_for_test();
        window.activate_window();
        let slider = cx.new(|_| SliderState::new());
        let host = cx.new(|cx| Host {
            slider,
            vertical: false,
            disabled: false,
            extent: 200.,
            outside: cx.focus_handle(),
            events: Vec::new(),
            bubbled: 0,
            subscription: None,
        });
        captured.replace(Some(host.clone()));
        Root::new(host, window, cx)
    });
    let host = captured.borrow_mut().take().unwrap();
    let reference: Value =
        serde_json::from_str(include_str!("../assets/slider-reference.json")).unwrap();
    for case in reference["keys"].as_array().unwrap() {
        cx.update(|window, cx| {
            host.update(cx, |host, cx| {
                host.configure(case, cx);
                window.focus(&host.outside);
            })
        });
        cx.run_until_parked();
        if case["disabled"] == false {
            cx.update(|window, cx| {
                let id = window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .find(|(_, node)| node.role() == Role::Slider)
                    .unwrap()
                    .0;
                window.dispatch_accessibility_action_for_test(
                    ActionRequest {
                        action: Action::Focus,
                        target_tree: TreeId::ROOT,
                        target_node: id,
                        data: None,
                    },
                    cx,
                );
            });
        }
        let key = match case["key"].as_str().unwrap() {
            "ArrowUp" => "up",
            "ArrowDown" => "down",
            "ArrowRight" => "right",
            "ArrowLeft" => "left",
            "Home" => "home",
            "End" => "end",
            other => other,
        };
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        host.read_with(cx, |host, cx| {
            let actual = f64::from(host.slider.read(cx).value().end());
            let expected = case["expected"].as_f64().unwrap();
            assert!(
                (actual - expected).abs() < 0.00001,
                "Keyboard {case}: got {actual}"
            );
            assert_eq!(
                host.events.iter().any(|event| event.0),
                case["ended"].as_bool().unwrap(),
                "{case}"
            );
            assert_eq!(
                host.bubbled == 0,
                case["prevented"].as_bool().unwrap(),
                "{case}"
            );
        });
    }
    for case in reference["pointers"].as_array().unwrap() {
        cx.update(|window, cx| {
            host.update(cx, |host, cx| {
                host.configure(case, cx);
                window.focus(&host.outside);
            })
        });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("slider-fixture").unwrap();
        let fraction = case["fraction"].as_f64().unwrap() as f32;
        let target = if case["vertical"] == true {
            point(
                bounds.center().x,
                bounds.top() + bounds.size.height * fraction,
            )
        } else {
            point(
                bounds.left() + bounds.size.width * fraction,
                bounds.center().y,
            )
        };
        cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        // Movement is delivered even below the drag-and-drop threshold.
        cx.simulate_mouse_move(target, Some(MouseButton::Left), Modifiers::none());
        cx.run_until_parked();
        cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        host.read_with(cx, |host, cx| {
            let actual = f64::from(host.slider.read(cx).value().end());
            let expected = case["expected"]
                .as_f64()
                .unwrap_or(case["min"].as_f64().unwrap());
            assert!(
                (actual - expected).abs() < 0.00001,
                "Pointer {case}: got {actual}"
            );
            assert_eq!(
                host.events.iter().any(|event| event.0),
                case["disabled"] == false,
                "{case}"
            );
        });
    }
    for rtl in [false, true] {
        let case =
            serde_json::json!({"min":0,"max":10,"step":1,"value":3,"rtl":rtl,"disabled":false});
        cx.update(|_, cx| host.update(cx, |host, cx| host.configure(&case, cx)));
        for extent in [200., 320.] {
            cx.update(|_, cx| {
                host.update(cx, |host, cx| {
                    host.extent = extent;
                    cx.notify();
                })
            });
            cx.run_until_parked();
            let bounds = cx.debug_bounds("slider-fixture").unwrap();
            cx.update(|window, cx| {
                let scale = window.scale_factor();
                let quads = window.painted_lines_and_quads_for_test();
                let start =
                    f32::from(bounds.left() + bounds.size.width * if rtl { 0.7 } else { 0. })
                        * scale;
                let thumb =
                    f32::from(bounds.left() + bounds.size.width * if rtl { 0.7 } else { 0.3 })
                        * scale;
                assert!(
                    quads
                        .iter()
                        .any(|(quad, color, _, _)| *color == cx.theme().slider_bar
                            && (f64::from(quad.origin.x) - f64::from(start)).abs() < 1.
                            && (f64::from(quad.size.width)
                                - f64::from(bounds.size.width) * f64::from(scale) * 0.3)
                                .abs()
                                < 1.
                            && f64::from(quad.size.height) <= f64::from(scale) * 6.1),
                    "Missing mirrored fill: {rtl}"
                );
                assert!(
                    quads
                        .iter()
                        .any(|(quad, color, _, _)| *color == cx.theme().slider_thumb
                            && (f64::from(quad.center().x) - f64::from(thumb)).abs() < 1.
                            && (f64::from(quad.size.width) - f64::from(scale) * 14.).abs() < 1.),
                    "Missing mirrored thumb: {rtl}"
                );
            });
        }
    }
}
