use super::*;
use ::core::prelude::v1::test;
use gpui_component::{Disableable, Root, button::ButtonVariants};

#[gpui::test]
fn wrapped_button_transitions_match_original_painted_colors(cx: &mut TestAppContext) {
    struct Probe {
        variant: String,
        disabled: bool,
        outside: FocusHandle,
        button: FocusHandle,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .track_focus(&self.outside)
                .p(px(30.))
                .child(
                    super::button("motion-button", cx)
                        .debug_selector(|| "motion-button".into())
                        .focus_handle(&self.button)
                        .label("Button")
                        .disabled(self.disabled)
                        .map(|button| match self.variant.as_str() {
                            "filled" => button.primary(),
                            "subtle" => button.ghost(),
                            "outline" => button.outline(),
                            "transparent" => button.text(),
                            "state-error" => button.danger(),
                            "state-info" => button.info(),
                            "state-success" => button.success(),
                            "state-warning" => button.warning(),
                            _ => button,
                        }),
                )
        }
    }
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    let mut cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../assets/button-motion-reference.json")).unwrap();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../assets/button-styles-reference.json")).unwrap();
    let custom: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../assets/button-custom-motion-reference.json"
    ))
    .unwrap();
    cases.extend(custom.into_iter().map(|mut case| {
        case["mode"] = "dark".into();
        case["variant"] = "default".into();
        case
    }));
    for case in cases.iter().filter(|case| case["variant"] != "secondary") {
        let custom = case["declarations"].as_str().map(|declarations| {
            let declarations = declarations.replace("#button", "#motion-button");
            format!("#motion-button{{background:#103050;color:#eeeeee;border:1px solid #305070;opacity:1;{declarations}}}#motion-button:hover{{background:#e08030;color:#3050a0;border-color:#d04080;opacity:.4}}"
        )}).unwrap_or_default();
        cx.update(|cx| {
            let palette = appearance::Palette {
                mode: case["mode"].as_str().unwrap().into(),
                ..Default::default()
            };
            let mut variables = appearance::css_variables(&palette);
            variables.extend(
                serde_json::from_value::<BTreeMap<String, String>>(reference["variables"].clone())
                    .unwrap(),
            );
            variables.insert("--mantine-primary-color-filled".into(), "#0088cc".into());
            variables.insert("--mantine-primary-color-contrast".into(), "#ffffff".into());
            variables.insert(
                "--mantine-color-primary-outline-hover".into(),
                "rgba(0,136,204,.05)".into(),
            );
            cx.set_global(
                NativeStyles::new(
                    Stylesheet::parse([super::button_stylesheet().as_str(), custom.as_str()])
                        .unwrap(),
                    palette,
                )
                .with_variables(variables),
            );
        });
        let (root, cx) = cx.add_window_view(|window, cx| {
            let outside = cx.focus_handle();
            outside.focus(window);
            let probe = cx.new(|cx| Probe {
                variant: case["variant"].as_str().unwrap().into(),
                disabled: false,
                outside,
                button: cx.focus_handle(),
            });
            Root::new(probe, window, cx)
        });
        let probe = root.read_with(cx, |root, _| {
            root.view().clone().downcast::<Probe>().unwrap()
        });
        cx.run_until_parked();
        let bounds = cx.debug_bounds("motion-button").unwrap();
        if case["phase"] == "leave" || case["phase"] == "reverse" {
            cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(
                    if case["phase"] == "reverse" { 123 } else { 800 },
                ));
            cx.refresh().unwrap();
            cx.run_until_parked();
            cx.simulate_mouse_move(point(px(0.), px(0.)), None, Modifiers::none());
        } else if case["phase"] == "hover" {
            cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
        } else if case["phase"] == "focus" {
            cx.update(|window, cx| probe.read(cx).button.focus(window));
        } else {
            probe.update(cx, |probe, cx| {
                probe.disabled = true;
                cx.notify();
            });
        }
        cx.run_until_parked();
        let mut previous = 0;
        for frame in case["frames"].as_array().unwrap() {
            let elapsed = frame["elapsed"].as_u64().unwrap();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(elapsed - previous));
            previous = elapsed;
            cx.refresh().unwrap();
            cx.run_until_parked();
            let background = cx
                .update(|window, _| window.painted_quad_opacities_for_test())
                .into_iter()
                .find_map(|(paint, color)| (paint == bounds).then_some(color))
                .unwrap_or_else(transparent_black);
            let border = cx
                .update(|window, _| window.painted_boxes_for_test())
                .into_iter()
                .find_map(|(paint, borders, _, color)| {
                    (paint == bounds && borders.top > px(0.)).then_some(color)
                })
                .unwrap_or_else(transparent_black);
            let foreground = cx
                .update(|window, _| window.painted_glyphs_with_opacity_for_test())
                .into_iter()
                .map(|(_, _, color)| color)
                .next()
                .expect("Button text");
            for (key, actual) in [
                ("background", background),
                ("border", border),
                ("color", foreground),
            ] {
                let mut expected: [f32; 4] = if let Some(pixels) = frame["paint"][key].as_array() {
                    std::array::from_fn(|index| pixels[index].as_f64().unwrap() as f32 / 255.)
                } else {
                    appearance::theme_color(
                        frame[key].as_str().unwrap(),
                        &appearance::Palette::default(),
                    )
                    .unwrap()
                };
                if key != "border" {
                    expected[3] *= frame["opacity"].as_f64().unwrap() as f32;
                }
                let actual = actual.to_rgb();
                let difference = [
                    actual.r * actual.a - expected[0] * expected[3],
                    actual.g * actual.a - expected[1] * expected[3],
                    actual.b * actual.a - expected[2] * expected[3],
                    actual.a - expected[3],
                ]
                .into_iter()
                .map(f32::abs)
                .fold(0., f32::max);
                assert!(
                    difference <= 2. / 255.,
                    "{} {} {} at {elapsed}ms, {key}: {actual:?}, expected {expected:?}, difference {difference}; CSS {custom}",
                    case["mode"],
                    case["variant"],
                    case["phase"]
                );
            }
        }
        cx.update(|window, _| window.remove_window());
    }
}
