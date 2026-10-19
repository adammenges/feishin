use ::core::prelude::v1::test;
use gpui::{prelude::*, *};
use gpui_component::{
    Root, StyledExt,
    button::Button,
    input::{Input, InputState},
};
use std::{cell::Cell, rc::Rc};

struct Contents {
    focus: FocusHandle,
    input: Entity<InputState>,
    clicks: Rc<Cell<usize>>,
    hovered: Rc<Cell<bool>>,
    tooltips: Rc<Cell<usize>>,
}
impl Render for Contents {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let click = |id: &'static str, label: &'static str, focus: Option<&FocusHandle>| {
            let clicks = self.clicks.clone();
            let hovered = self.hovered.clone();
            Button::new(id)
                .debug_selector(move || id.into())
                .label(label)
                .h(px(30.))
                .when_some(focus, |button, focus| button.focus_handle(focus))
                .when(id == "inert-child", |button| {
                    button.on_hover(move |value, _, _| hovered.set(*value))
                })
                .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
        };
        let tooltips = self.tooltips.clone();
        div()
            .id("inert-body")
            .tooltip(move |window, cx| {
                tooltips.set(tooltips.get() + 1);
                gpui_component::tooltip::Tooltip::new("Closing tooltip").build(window, cx)
            })
            .v_flex()
            .w(px(200.))
            .h(px(120.))
            .bg(rgb(0x123456))
            .child(click("inert-child", "Inside", Some(&self.focus)))
            .child(Input::new(&self.input))
            .child(
                deferred(div().child(deferred(click("inert-overlay", "Overlay", None))))
                    .with_priority(20),
            )
    }
}

struct Probe {
    inert: bool,
    cached: bool,
    before: FocusHandle,
    after: FocusHandle,
    contents: Entity<Contents>,
}
impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let content = if self.cached {
            AnyView::from(self.contents.clone())
                .cached(StyleRefinement::default().w(px(200.)).h(px(120.)))
                .into_any_element()
        } else {
            self.contents.clone().into_any_element()
        };
        div()
            .v_flex()
            .w(px(200.))
            .child(
                Button::new("before")
                    .label("Before")
                    .focus_handle(&self.before)
                    .h(px(30.)),
            )
            .child(div().inert(self.inert).child(content))
            .child(
                Button::new("after")
                    .label("After")
                    .focus_handle(&self.after)
                    .h(px(30.)),
            )
    }
}

#[gpui::test]
fn closing_sections_exclude_input_tab_stops_and_accessibility_while_still_painted(
    cx: &mut TestAppContext,
) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    for cached in [false, true] {
        let clicks = Rc::new(Cell::new(0));
        let captured = Rc::new(std::cell::RefCell::new(None));
        let (_, window_cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let input = cx.new(|cx| InputState::new(window, cx));
            let contents = cx.new(|cx| Contents {
                focus: cx.focus_handle(),
                input,
                clicks: clicks.clone(),
                hovered: Rc::new(Cell::new(false)),
                tooltips: Rc::new(Cell::new(0)),
            });
            let probe = cx.new(|cx| Probe {
                inert: false,
                cached,
                before: cx.focus_handle(),
                after: cx.focus_handle(),
                contents,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        window_cx.simulate_resize(size(px(500.), px(500.)));
        window_cx.run_until_parked();
        for inert in [false, true, true, false] {
            probe.update(window_cx, |probe, cx| {
                probe.inert = inert;
                cx.notify();
            });
            window_cx.run_until_parked();
            window_cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                for label in ["Inside", "Overlay"] {
                    assert_eq!(
                        tree.nodes
                            .iter()
                            .any(|(_, node)| node.role() == Role::Button
                                && node.label() == Some(label)),
                        !inert,
                        "accessibility {label}, cached {cached}"
                    );
                }
                assert!(
                    window
                        .painted_quad_opacities_for_test()
                        .iter()
                        .any(|(_, color)| *color == Hsla::from(rgb(0x123456))),
                    "contents should stay painted"
                );
            });
            for selector in ["inert-child", "inert-overlay"] {
                let target = window_cx.debug_bounds(selector).unwrap().center();
                let before = clicks.get();
                window_cx.simulate_click(target, Modifiers::none());
                window_cx.run_until_parked();
                assert_eq!(
                    clicks.get() - before,
                    usize::from(!inert),
                    "click {selector}, cached {cached}"
                );
            }
            window_cx.update(|window, cx| {
                let probe = probe.read(cx);
                probe.before.focus(window);
                window.focus_next();
                assert!(
                    if inert {
                        probe.after.is_focused(window)
                    } else {
                        probe.contents.read(cx).focus.is_focused(window)
                    },
                    "tab order, cached {cached}"
                );
            });
            let input =
                probe.read_with(window_cx, |probe, cx| probe.contents.read(cx).input.clone());
            window_cx.update(|window, cx| input.update(cx, |input, cx| input.focus(window, cx)));
            window_cx.run_until_parked();
            assert_eq!(
                window_cx.update(|window, cx| input.read(cx).focus_handle(cx).is_focused(window)),
                !inert,
                "inert content must release focus, cached {cached}"
            );
            let before = input.read_with(window_cx, |input, _| input.value());
            window_cx.simulate_input("x");
            assert_eq!(
                input
                    .read_with(window_cx, |input, _| input.value())
                    .as_str(),
                if inert {
                    before.to_string()
                } else {
                    format!("{before}x")
                },
                "text input, cached {cached}"
            );
        }
        let target = window_cx.debug_bounds("inert-child").unwrap().center();
        let hovered = probe.read_with(window_cx, |probe, cx| {
            probe.contents.read(cx).hovered.clone()
        });
        window_cx.simulate_mouse_move(target, None, Modifiers::none());
        window_cx.run_until_parked();
        assert!(hovered.get());
        window_cx.simulate_mouse_down(target, MouseButton::Left, Modifiers::none());
        probe.update(window_cx, |probe, cx| {
            probe.inert = true;
            cx.notify();
        });
        window_cx.run_until_parked();
        let hover_cleared = !hovered.get();
        window_cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        probe.update(window_cx, |probe, cx| {
            probe.inert = false;
            cx.notify();
        });
        window_cx.run_until_parked();
        let before = clicks.get();
        window_cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        window_cx.run_until_parked();
        assert_eq!(
            clicks.get(),
            before,
            "closing must cancel a pending press, cached {cached}"
        );
        assert!(hover_cleared, "closing must clear hover, cached {cached}");
        let tooltips = probe.read_with(window_cx, |probe, cx| {
            probe.contents.read(cx).tooltips.clone()
        });
        for visible in [false, true] {
            window_cx.simulate_mouse_move(point(px(450.), px(450.)), None, Modifiers::none());
            window_cx.run_until_parked();
            let before = tooltips.get();
            window_cx.simulate_mouse_move(target, None, Modifiers::none());
            window_cx.run_until_parked();
            window_cx
                .executor()
                .advance_clock(std::time::Duration::from_millis(if visible {
                    600
                } else {
                    100
                }));
            window_cx.run_until_parked();
            assert_eq!(tooltips.get(), before + usize::from(visible));
            probe.update(window_cx, |probe, cx| {
                probe.inert = true;
                cx.notify();
            });
            window_cx.run_until_parked();
            window_cx
                .executor()
                .advance_clock(std::time::Duration::from_millis(600));
            window_cx.run_until_parked();
            assert_eq!(
                tooltips.get(),
                before + usize::from(visible),
                "pending tooltip, cached {cached}"
            );
            assert!(
                !window_cx.update(|window, _| window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some("Closing tooltip"))),
                "visible tooltip, cached {cached}"
            );
            probe.update(window_cx, |probe, cx| {
                probe.inert = false;
                cx.notify();
            });
            window_cx.run_until_parked();
        }
        window_cx.update(|window, _| window.remove_window());
    }
}
