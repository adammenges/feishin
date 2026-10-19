use super::*;
use ::core::prelude::v1::test;
use gpui_component::{Icon, IconName, Root, Sizable, Size};
use std::{cell::Cell, rc::Rc};

#[gpui::test]
fn custom_theme_button_sizes_match_original(cx: &mut TestAppContext) {
    struct Probe {
        size: String,
        icon: bool,
        font: Rc<Cell<f32>>,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let font = self.font.clone();
            div().flex().items_start().p_8().child(
                super::button("layout-button", cx)
                    .debug_selector(|| "layout-button".into())
                    .with_size(match self.size.trim_start_matches("compact-") {
                        "xs" => Size::XSmall,
                        "sm" => Size::Small,
                        "md" => Size::Medium,
                        _ => Size::Large,
                    })
                    .when(self.size.starts_with("compact-"), |button| button.compact())
                    .when(self.icon, |button| {
                        button.icon(Icon::new(IconName::Check).with_size(px(16.)))
                    })
                    .child(
                        div()
                            .debug_selector(|| "layout-label".into())
                            .child("Button")
                            .child(
                                canvas(
                                    move |_, window, _| {
                                        font.set(f32::from(
                                            window
                                                .text_style()
                                                .font_size
                                                .to_pixels(window.rem_size()),
                                        ));
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .size_0(),
                            ),
                    ),
            )
        }
    }
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../assets/button-layout-reference.json")).unwrap();
    for case in cases {
        cx.update(|cx| {
            gpui_component::Theme::global_mut(cx).font_size =
                px(case["rem"].as_f64().unwrap() as f32);
            let palette = appearance::Palette {
                mantine_override: case["override"].clone(),
                ..appearance::builtins()["defaultTheme"].clone()
            };
            cx.set_global(NativeStyles::new(
                Stylesheet::parse([super::button_stylesheet().as_str()]).unwrap(),
                palette,
            ));
        });
        let font = Rc::new(Cell::new(0.));
        let (_, window_cx) = cx.add_window_view(|window, cx| {
            let probe = cx.new(|_| Probe {
                size: case["size"].as_str().unwrap().into(),
                icon: case["icon"].as_bool().unwrap(),
                font: font.clone(),
            });
            Root::new(probe, window, cx)
        });
        window_cx.run_until_parked();
        let button = window_cx.debug_bounds("layout-button").unwrap();
        let label = window_cx.debug_bounds("layout-label").unwrap();
        let close = |name: &str, actual: f32, tolerance: f32| {
            assert!(
                (actual - case[name].as_f64().unwrap() as f32).abs() <= tolerance,
                "{name}: {actual}; {case}"
            );
        };
        close("height", f32::from(button.size.height), 1.);
        close("contentOffset", f32::from(label.left() - button.left()), 1.);
        close("fontSize", font.get(), 0.001);
        // CSS preserves the specified radius in computed styles, then reduces
        // overlapping corners at paint time.
        let radius = (case["radius"].as_f64().unwrap() as f32)
            .min(f32::from(button.size.height) / 2.)
            .min(f32::from(button.size.width) / 2.);
        assert!(
            window_cx
                .update(|window, _| window.painted_boxes_for_test())
                .iter()
                .any(|(bounds, _, corners, _)| *bounds == button
                    && (f32::from(corners.top_left) - radius).abs() < 0.001),
            "radius: {case}; {:?}",
            window_cx.update(|window, _| window.painted_boxes_for_test())
        );
        window_cx.update(|window, _| window.remove_window());
    }
}
