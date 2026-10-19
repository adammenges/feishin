use super::*;
use ::core::prelude::v1::test;
use gpui_component::{Disableable, Icon, IconName, Root, Sizable, Size, button::ButtonVariants};
use std::{cell::Cell, rc::Rc, time::Duration};

struct Probe {
    case: serde_json::Value,
    loading: bool,
    clicks: Rc<Cell<usize>>,
}
impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = self.case["size"].as_str().unwrap();
        let clicks = self.clicks.clone();
        let button = super::button("loading-button", cx)
            .debug_selector(|| "loading-button".into())
            .label("Button")
            .with_size(match size.trim_start_matches("compact-") {
                "xs" => Size::XSmall,
                "sm" => Size::Small,
                "md" => Size::Medium,
                _ => Size::Large,
            })
            .when(size.starts_with("compact-"), |button| button.compact())
            .when(self.case["icon"] == true, |button| {
                button.icon(Icon::new(IconName::Check).with_size(px(16.)))
            })
            .disabled(self.case["explicitDisabled"] == true)
            .loading(self.loading)
            .on_click(move |_, _, _| clicks.set(clicks.get() + 1));
        div()
            .p_8()
            .flex()
            .items_start()
            .child(match self.case["variant"].as_str().unwrap() {
                "filled" => button.primary(),
                "outline" => button.outline(),
                "transparent" => button.text(),
                _ => button,
            })
    }
}

fn theme(cx: &mut TestAppContext, case: &serde_json::Value) {
    let rem = case["rem"].as_f64().unwrap() as f32;
    let mode = case["mode"].as_str().unwrap_or("dark");
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../assets/button-styles-reference.json")).unwrap();
    cx.update(|cx| {
        gpui_component::Theme::global_mut(cx).font_size = px(rem);
        gpui_component::Theme::global_mut(cx).font_family = "Inter".into();
        gpui_component::Theme::global_mut(cx).primary = gpui::rgb(0x0088cc).into();
        gpui_component::Theme::global_mut(cx).mode = if mode == "light" {
            gpui_component::ThemeMode::Light
        } else {
            gpui_component::ThemeMode::Dark
        };
        let palette = appearance::Palette {
            mode: mode.into(),
            mantine_override: case
                .get("override")
                .cloned()
                .unwrap_or(serde_json::json!({})),
            ..Default::default()
        };
        let mut variables = appearance::css_variables(&palette);
        variables.extend(
            serde_json::from_value::<BTreeMap<String, String>>(reference["variables"].clone())
                .unwrap(),
        );
        variables.insert("--mantine-primary-color-filled".into(), "#0088cc".into());
        variables.insert("--mantine-primary-color-contrast".into(), "#ffffff".into());
        cx.set_global(
            NativeStyles::new(
                Stylesheet::parse([
                    super::button_stylesheet().as_str(),
                    super::icon_stylesheet().as_str(),
                ])
                .unwrap(),
                palette,
            )
            .with_variables(variables),
        );
    });
}

#[gpui::test]
fn wrapped_loading_preserves_geometry_disables_activation_and_matches_original(
    cx: &mut TestAppContext,
) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/button-loading-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/button-loading-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for case in cases.iter().filter(|case| case["loading"] == true) {
        theme(cx, case);
        let clicks = Rc::new(Cell::new(0));
        let (root, window_cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let probe = cx.new(|_| Probe {
                case: case.clone(),
                loading: true,
                clicks: clicks.clone(),
            });
            Root::new(probe, window, cx)
        });
        let probe = root.read_with(window_cx, |root, _| {
            root.view().clone().downcast::<Probe>().unwrap()
        });
        window_cx.run_until_parked();
        let button = window_cx.debug_bounds("loading-button").unwrap();
        let glyphs = window_cx
            .update(|window, _| window.painted_glyphs_with_opacity_for_test())
            .into_iter()
            .filter(|(_, _, color)| color.a > 0.001)
            .collect::<Vec<_>>();
        assert_eq!(
            glyphs.len(),
            1,
            "Only the spinner is painted: {case}; {glyphs:?}"
        );
        let (spinner, _, color) = glyphs[0];
        let expected = &case["loader"]["svg"];
        // GPUI rounds the SVG raster to its physical-pixel size.
        assert!(
            (f32::from(spinner.size.width) - expected[2].as_f64().unwrap() as f32).abs() <= 0.5,
            "{case}; {spinner:?}"
        );
        assert!(
            (spinner.center().x - button.center().x).abs() <= px(0.5),
            "{case}; {spinner:?}"
        );
        assert!(
            (f32::from(spinner.top() - button.top()) - expected[1].as_f64().unwrap() as f32).abs()
                <= 1.,
            "{case}; {spinner:?}; {button:?}"
        );
        let expected_color = super::color(
            case["loader"]["color"].as_str().unwrap(),
            &appearance::Palette::default(),
        )
        .unwrap();
        assert!(
            (color.a - case["opacity"].as_f64().unwrap() as f32).abs() < 0.001,
            "{case}; {color:?}"
        );
        assert!(
            (color.to_rgb().r - expected_color.to_rgb().r)
                .abs()
                .max((color.to_rgb().g - expected_color.to_rgb().g).abs())
                .max((color.to_rgb().b - expected_color.to_rgb().b).abs())
                < 0.001,
            "{case}; {color:?}"
        );
        assert!(window_cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Button && node.is_disabled())
        }));
        window_cx.simulate_click(button.center(), Modifiers::none());
        assert_eq!(clicks.get(), 0);
        probe.update(window_cx, |probe, cx| {
            probe.loading = false;
            cx.notify()
        });
        window_cx.run_until_parked();
        window_cx
            .executor()
            .advance_clock(Duration::from_millis(160));
        window_cx.refresh().unwrap();
        window_cx.run_until_parked();
        assert_eq!(
            window_cx.debug_bounds("loading-button").unwrap().size,
            button.size,
            "Loading changed the width: {case}"
        );
        assert!(
            window_cx
                .update(|window, _| window.painted_glyphs_for_test())
                .len()
                > 1,
            "Label did not return: {case}"
        );
        window_cx.simulate_click(button.center(), Modifiers::none());
        assert_eq!(clicks.get(), usize::from(case["explicitDisabled"] != true));
        window_cx.update(|window, _| window.remove_window());
    }
}

#[gpui::test]
fn wrapped_loading_motion_matches_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/button-loading-motion-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/button-loading-motion-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for case in cases {
        theme(cx, &case);
        let entering = case["phase"] == "enter";
        let (root, window_cx) = cx.add_window_view(|window, cx| {
            let probe = cx.new(|_| Probe {
                case: serde_json::json!({"size":"lg","variant":"default","icon":true}),
                loading: !entering,
                clicks: Rc::new(Cell::new(0)),
            });
            Root::new(probe, window, cx)
        });
        let probe = root.read_with(window_cx, |root, _| {
            root.view().clone().downcast::<Probe>().unwrap()
        });
        window_cx.run_until_parked();
        let button = window_cx.debug_bounds("loading-button").unwrap();
        let label_bounds = window_cx
            .update(|window, _| window.painted_glyphs_for_test())
            .into_iter()
            .find(|(bounds, _, _)| {
                bounds.size.width < px(case["rem"].as_f64().unwrap() as f32 * 0.5)
            })
            .unwrap()
            .0;
        probe.update(window_cx, |probe, cx| {
            probe.loading = entering;
            cx.notify()
        });
        window_cx.run_until_parked();
        let mut previous = 0;
        for frame in case["frames"].as_array().unwrap() {
            let elapsed = frame["elapsed"].as_u64().unwrap();
            window_cx
                .executor()
                .advance_clock(Duration::from_millis(elapsed - previous));
            previous = elapsed;
            window_cx.refresh().unwrap();
            window_cx.run_until_parked();
            let glyphs =
                window_cx.update(|window, _| window.painted_glyphs_with_opacity_for_test());
            let label_opacity = frame["innerOpacity"].as_f64().unwrap() as f32
                * frame["labelOpacity"].as_f64().unwrap() as f32;
            let actual_label = glyphs
                .iter()
                .find(|(bounds, _, _)| *bounds == label_bounds)
                .map_or(0., |(_, _, color)| color.a);
            assert!(
                (actual_label - label_opacity).abs() < 0.001,
                "label opacity {case}; {frame}; {glyphs:?}"
            );
            let expected_overlay = frame["overlayOpacity"].as_f64().unwrap() as f32 * 0.15;
            let light = case["mode"] == "light";
            let overlay = window_cx
                .update(|window, _| window.painted_quad_opacities_for_test())
                .into_iter()
                .find(|(bounds, color)| {
                    let rgb = color.to_rgb();
                    (bounds.size.width - button.size.width).abs() < px(1.)
                        && color.a <= 0.151
                        && if light {
                            rgb.r > 0.999 && rgb.g > 0.999 && rgb.b > 0.999
                        } else {
                            rgb.r < 0.001 && rgb.g < 0.001 && rgb.b < 0.001
                        }
                })
                .map_or(0., |(_, color)| color.a);
            assert!(
                (overlay - expected_overlay).abs() < 0.001,
                "overlay opacity {case}; {frame}; {overlay}"
            );
            if !entering && elapsed == 150 {
                continue;
            }
            let bounds = window_cx
                .debug_bounds("wrapped-button-loader")
                .unwrap_or_else(|| panic!("Loader is missing: {case}; {frame}"));
            let transform: Vec<f32> = frame["loaderTransform"]
                .as_str()
                .unwrap()
                .trim_start_matches("matrix(")
                .trim_end_matches(')')
                .split(',')
                .map(|value| value.trim().parse().unwrap())
                .collect();
            assert!(
                (f32::from(bounds.top() - button.center().y) - transform[5]).abs() < 2.,
                "{case}; {frame}; {bounds:?}"
            );
            let opacity = frame["loaderOpacity"].as_f64().unwrap() as f32;
            let glyphs =
                window_cx.update(|window, _| window.painted_glyphs_with_opacity_for_test());
            if elapsed >= 50 && opacity > 0. {
                assert!(
                    glyphs
                        .iter()
                        .any(|(_, _, color)| (color.a - opacity).abs() < 0.001),
                    "loader opacity {frame}; {glyphs:?}"
                );
            }
        }
        window_cx.update(|window, _| window.remove_window());
    }
}
