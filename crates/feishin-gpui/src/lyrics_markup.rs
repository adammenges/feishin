use feishin_core::lyrics_markup::Span;
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, StyledExt};

#[derive(IntoElement)]
pub(crate) struct Text(pub String);
impl RenderOnce for Text {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        content(&self.0, window, cx).0
    }
}

pub(crate) fn content(
    text: &str,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, Vec<TextLayout>) {
    content_with_readings(text, window, cx, false)
}
pub(crate) fn karaoke_content(
    text: &str,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, Vec<TextLayout>) {
    content_with_readings(text, window, cx, true)
}
fn content_with_readings(
    text: &str,
    window: &mut Window,
    cx: &mut App,
    karaoke: bool,
) -> (AnyElement, Vec<TextLayout>) {
    let link_color = rgb(if cx.theme().mode.is_dark() {
        0x9e9eff
    } else {
        0x0000ee
    })
    .into();
    let mut blocks = feishin_core::lyrics_markup::parse_layout(text);
    if blocks.len() == 1 && blocks[0].margin_before == 0. {
        return inline_content(blocks.pop().unwrap().spans, window, link_color, karaoke);
    }
    let font_size = window.text_style().font_size.to_pixels(window.rem_size());
    let mut layouts = vec![];
    let children = blocks
        .into_iter()
        .enumerate()
        .map(|(index, block)| {
            let (content, shapes) = inline_content(block.spans, window, link_color, karaoke);
            layouts.extend(shapes);
            div()
                .id(("lyric-markup-block", index))
                .mt(font_size * block.margin_before)
                .child(content)
        })
        .collect::<Vec<_>>();
    (
        div()
            .v_flex()
            .min_w_0()
            .children(children)
            .into_any_element(),
        layouts,
    )
}

fn inline_content(
    spans: Vec<Span>,
    window: &mut Window,
    link_color: Hsla,
    karaoke: bool,
) -> (AnyElement, Vec<TextLayout>) {
    if spans.iter().all(|span| span.text.is_empty()) {
        return (div().into_any_element(), vec![]);
    }
    let font_size = window.text_style().font_size.to_pixels(window.rem_size());
    let alignment = window.text_style().text_align;
    let bold_weight = if window.text_style().font_weight.0 <= 500. {
        FontWeight::BOLD
    } else {
        FontWeight::BLACK
    };
    let mut layouts = vec![];
    if spans.iter().all(|span| span.ruby.is_none()) {
        let element = styled(&spans, &mut layouts, bold_weight, 0, link_color);
        return (element, layouts);
    }
    let mut children = vec![];
    let mut run = vec![];
    for (span_index, span) in spans.iter().enumerate() {
        if let Some(reading) = &span.ruby {
            if !run.is_empty() {
                children.push(styled(
                    &run,
                    &mut layouts,
                    bold_weight,
                    children.len(),
                    link_color,
                ));
                run.clear();
            }
            let mut reading_font = window.text_style().font();
            if span.bold {
                reading_font.weight = bold_weight;
            }
            if span.italic {
                reading_font.style = FontStyle::Italic;
            }
            let base_font = reading_font.clone();
            if karaoke {
                reading_font.weight = FontWeight::MEDIUM;
            }
            let reading_size = font_size * if karaoke { 0.55 } else { 0.5 };
            let mut expansion = px(0.);
            let mut overhang_left = px(0.);
            let mut overhang_right = px(0.);
            if !reading.is_empty() && !span.text.contains('\n') && !reading.contains('\n') {
                let mut run = window.text_style().to_run(reading.len());
                run.font = reading_font.clone();
                let annotation = window.text_system().shape_line(
                    reading.clone().into(),
                    reading_size,
                    &[run.clone()],
                    None,
                );
                let units = annotation
                    .runs
                    .iter()
                    .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.index))
                    .collect::<std::collections::BTreeSet<_>>()
                    .len();
                run.len = span.text.len();
                run.font = base_font;
                let base_width = window
                    .text_system()
                    .shape_line(span.text.clone().into(), font_size, &[run], None)
                    .width;
                if units > 0
                    && reading.chars().all(|ch| {
                        matches!(ch as u32,
                    0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fcf | 0xf900..=0xfaff)
                    })
                {
                    expansion = (base_width - annotation.width).max(px(0.)) / units as f32;
                }
                let excess = (annotation.width - base_width).max(px(0.)) * 0.5;
                let adjacent_overhang = |neighbor: Option<&Span>, left: bool| {
                    let Some(neighbor) = neighbor.filter(|neighbor| neighbor.ruby.is_none()) else {
                        return px(0.);
                    };
                    let edge = if left {
                        neighbor.text.char_indices().next_back()
                    } else {
                        neighbor.text.char_indices().next()
                    };
                    let Some((index, ch)) = edge else {
                        return px(0.);
                    };
                    let whitespace = matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0c');
                    let text = if whitespace { " " } else { &neighbor.text };
                    if text.contains('\n') {
                        return px(0.);
                    }
                    let mut run = window.text_style().to_run(text.len());
                    if neighbor.bold {
                        run.font.weight = bold_weight;
                    }
                    if neighbor.italic {
                        run.font.style = FontStyle::Italic;
                    }
                    let shaped = window.text_system().shape_line(
                        text.to_owned().into(),
                        font_size,
                        &[run],
                        None,
                    );
                    let width = if whitespace {
                        shaped.width
                    } else {
                        // Selection cells preserve the edge grapheme's shaped advance.
                        shaped
                            .selection_ranges(index..index + ch.len_utf8())
                            .iter()
                            .fold(px(0.), |width, range| width + range.end - range.start)
                    };
                    (width * 0.5).min(reading_size * 0.5)
                };
                overhang_left = excess.min(adjacent_overhang(
                    span_index.checked_sub(1).and_then(|index| spans.get(index)),
                    true,
                ));
                overhang_right = excess.min(adjacent_overhang(spans.get(span_index + 1), false));
            }
            let reading = StyledText::new(reading.clone());
            layouts.push(reading.layout().clone());
            let reading_height = if karaoke {
                reading_size
            } else {
                window.text_system().normal_line_height(
                    &reading_font,
                    reading_size,
                    window.scale_factor(),
                )
            };
            let reading_id = window.text_system().resolve_font(&reading_font);
            let cap = window.text_system().cap_height(reading_id, reading_size);
            // Reserve the visible reading height while allowing its font leading to overlap.
            let reserve = if cap > px(0.) { cap } else { reading_size };
            let reserve = f32::from(reserve)
                .round()
                .clamp(0., f32::from(reading_height));
            let base = styled(
                std::slice::from_ref(span),
                &mut layouts,
                bold_weight,
                children.len(),
                link_color,
            );
            children.push(
                div()
                    .v_flex()
                    .items_center()
                    .flex_shrink_0()
                    .max_w_full()
                    .ml(-overhang_left)
                    .mr(-overhang_right)
                    .child(
                        div()
                            .text_size(reading_size)
                            .font(reading_font)
                            .line_height(reading_height)
                            .mb(px(reserve) - reading_height)
                            .when(span.href.is_some(), |div| div.text_color(link_color))
                            .when_some(span.href.clone(), |div, href| {
                                div.cursor_pointer()
                                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                        cx.open_url(&href)
                                    })
                            })
                            .child(
                                div()
                                    .relative()
                                    .left(expansion * 0.5)
                                    .letter_spacing(
                                        window.text_style().letter_spacing
                                            + reading_size
                                                * window.text_style().letter_spacing_fraction
                                            + expansion,
                                    )
                                    .child(reading),
                            ),
                    )
                    .child(base)
                    .into_any_element(),
            );
        } else {
            run.push(span.clone());
        }
    }
    if !run.is_empty() {
        children.push(styled(
            &run,
            &mut layouts,
            bold_weight,
            children.len(),
            link_color,
        ));
    }
    (
        div()
            .h_flex()
            .min_w_0()
            .w_full()
            .flex_wrap()
            .items_end()
            .when(alignment == TextAlign::Center, |div| div.justify_center())
            .when(alignment == TextAlign::Right, |div| div.justify_end())
            .children(children)
            .into_any_element(),
        layouts,
    )
}

fn styled(
    spans: &[Span],
    layouts: &mut Vec<TextLayout>,
    bold_weight: FontWeight,
    index: usize,
    link_color: Hsla,
) -> AnyElement {
    let mut text = String::new();
    let mut highlights = vec![];
    let mut links = vec![];
    for span in spans {
        let start = text.len();
        text.push_str(&span.text);
        let range = start..text.len();
        highlights.push((
            range.clone(),
            HighlightStyle {
                color: span.href.as_ref().map(|_| link_color),
                font_weight: span.bold.then_some(bold_weight),
                font_style: span.italic.then_some(FontStyle::Italic),
                ..Default::default()
            },
        ));
        if let Some(href) = &span.href {
            links.push((range, href.clone(), span.text.clone()));
        }
    }
    let text = StyledText::new(text).with_highlights(highlights);
    let layout = text.layout().clone();
    layouts.push(layout.clone());
    let transform = std::rc::Rc::new(std::cell::Cell::new(None));
    let capture = transform.clone();
    div()
        .id(("lyric-markup-run", index))
        .min_w_0()
        .max_w_full()
        .child(text)
        .when(!links.is_empty(), |div| {
            div.relative()
                .cursor_pointer()
                .child(
                    canvas(
                        {
                            let links = links.clone();
                            let layout = layout.clone();
                            move |bounds, window, cx| {
                                let align = window.text_style().text_align;
                                capture.set(Some((
                                    bounds,
                                    window.transform_element_bounds(bounds),
                                    align,
                                )));
                                layout.link_controls(
                                    links.iter().map(|(range, _, label)| {
                                        (range.clone(), SharedString::from(label.clone()))
                                    }),
                                    align,
                                    window,
                                    cx,
                                )
                            }
                        },
                        |_, controls, window, cx| {
                            for mut control in controls {
                                control.paint(window, cx)
                            }
                        },
                    )
                    .absolute()
                    .inset_0()
                    .size_full(),
                )
                .on_mouse_up(MouseButton::Left, move |event, _, cx| {
                    let Some((bounds, visual, align)) = transform.get() else {
                        return;
                    };
                    let position = {
                        let scale = visual.size.width / bounds.size.width;
                        if scale.is_finite() && scale > 0. {
                            bounds.origin + (event.position - visual.origin) / scale
                        } else {
                            event.position
                        }
                    };
                    if let Some((_, href, _)) = links.iter().find(|(range, _, _)| {
                        layout
                            .range_bounds(range.clone(), align)
                            .iter()
                            .any(|bounds| bounds.contains(&position))
                    }) {
                        cx.open_url(href);
                    }
                })
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use gpui::{
        AppContext, Context, IntoElement, Render, TestAppContext, Window, div, prelude::*, px,
        relative,
    };
    use gpui_component::{ActiveTheme, Root, StyledExt};
    use std::{cell::RefCell, rc::Rc};
    #[gpui::test]
    fn pronunciation_readings_keep_the_enclosing_link_and_line_click(cx: &mut TestAppContext) {
        struct Probe {
            case: serde_json::Value,
            clicks: usize,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .id("ruby-link-probe")
                    .w(px(self.case["width"].as_f64().unwrap() as f32))
                    .px(px(16.))
                    .text_size(px(self.case["size"].as_f64().unwrap() as f32))
                    .font_family("Inter")
                    .line_height(relative(1.2))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_align(match self.case["alignment"].as_str().unwrap() {
                        "center" => gpui::TextAlign::Center,
                        "right" => gpui::TextAlign::Right,
                        _ => gpui::TextAlign::Left,
                    })
                    .on_click(cx.listener(|this, _, _, _| this.clicks += 1))
                    .child(super::Text(self.case["text"].as_str().unwrap().into()))
            }
        }
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "macos") {
            include_str!("../assets/lyrics-link-layout-darwin-reference.json")
        } else {
            include_str!("../assets/lyrics-link-layout-linux-reference.json")
        })
        .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let probe = cx.new(|_| Probe {
                case: reference["cases"][0].clone(),
                clicks: 0,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        for case in reference["cases"].as_array().unwrap() {
            cx.update(|window, cx| {
                probe.update(cx, |probe, cx| {
                    probe.case = case.clone();
                    probe.clicks = 0;
                    cx.notify();
                });
                cx.open_url("https://example.com/baseline");
                window.blur();
                window.refresh();
            });
            cx.run_until_parked();
            let names = cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .filter(|(_, node)| node.role() == gpui::accesskit::Role::Link)
                    .map(|(_, node)| node.label().unwrap().to_owned())
                    .collect::<Vec<_>>()
            });
            assert_eq!(
                serde_json::to_value(names).unwrap(),
                case["accessibleNames"]
            );
            let reading = cx.update(|window, _| window.painted_glyphs_for_test()[0].0.center());
            cx.simulate_click(reading, gpui::Modifiers::none());
            cx.run_until_parked();
            assert_eq!(
                cx.opened_url().as_deref(),
                case["readingUrl"].as_str(),
                "{case}"
            );
            probe.read_with(cx, |probe, _| assert_eq!(probe.clicks, 1));
            cx.update(|window, cx| {
                cx.open_url("https://example.com/baseline");
                window.focus_next();
            });
            cx.run_until_parked();
            cx.simulate_keystrokes("enter");
            cx.run_until_parked();
            assert_eq!(cx.opened_url().as_deref(), case["readingUrl"].as_str());
            probe.read_with(cx, |probe, _| assert_eq!(probe.clicks, 2));
        }
    }
    #[gpui::test]
    fn formatted_text_links_support_keyboard_and_accessibility(cx: &mut TestAppContext) {
        struct Probe(gpui::ScrollHandle);
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .id("formatted-link-probe")
                    .w(px(300.))
                    .h(px(120.))
                    .v_flex()
                    .overflow_y_scroll()
                    .track_scroll(&self.0)
                    .child(div().flex_shrink_0().child(gpui_component::text::TextView::html(
                        "biography-link",
                        "<p>Visit <a href='https://example.com/artist'>Artist biography</a>.</p>",
                        window,
                        cx,
                    )))
                    .child(div().h(px(400.)).flex_shrink_0())
                    .child(div().flex_shrink_0().child(gpui_component::text::TextView::markdown(
                        "release-link",
                        "[Release details](https://example.com/release)",
                        window,
                        cx,
                    )))
            }
        }
        cx.use_platform_text_system();
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let probe = cx.new(|_| Probe(gpui::ScrollHandle::new()));
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(250));
        cx.run_until_parked();
        let links = cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .filter(|(_, node)| node.role() == gpui::accesskit::Role::Link)
                .map(|(id, node)| (*id, node.label().unwrap().to_owned()))
                .collect::<Vec<_>>()
        });
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].1, "Artist biography");
        assert_eq!(links[1].1, "Release details");
        cx.update(|window, _| window.focus_next());
        cx.run_until_parked();
        cx.simulate_keystrokes("space");
        cx.run_until_parked();
        assert!(cx.opened_url().is_none());
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://example.com/artist")
        );
        cx.update(|window, _| window.focus_next());
        cx.run_until_parked();
        probe.read_with(cx, |probe, _| assert!(probe.0.offset().y < px(0.)));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        for _ in 0..2 {
            cx.update(|window, cx| window.draw_next_frame_for_test(cx));
            cx.run_until_parked();
        }
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://example.com/release")
        );
        cx.update(|window, cx| {
            window.dispatch_accessibility_action_for_test(
                gpui::accesskit::ActionRequest {
                    action: gpui::accesskit::Action::Click,
                    target_tree: gpui::accesskit::TreeId::ROOT,
                    target_node: links[0].0,
                    data: None,
                },
                cx,
            );
        });
        cx.run_until_parked();
        for _ in 0..2 {
            cx.update(|window, cx| window.draw_next_frame_for_test(cx));
            cx.run_until_parked();
        }
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://example.com/artist")
        );
    }
    #[gpui::test]
    fn focusing_an_offscreen_lyric_link_scrolls_it_into_view(cx: &mut TestAppContext) {
        struct Probe {
            scroll: gpui::ScrollHandle,
            reveals: usize,
            clicks: usize,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let reveal = cx.listener(|this, _: &(), _, _| this.reveals += 1);
                div()
                    .id("link-scroll-probe")
                    .w(px(300.))
                    .h(px(120.))
                    .v_flex()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll_reveal(move |window, cx| reveal(&(), window, cx))
                    .on_click(cx.listener(|this, _, _, _| this.clicks += 1))
                    .child(div().h(px(400.)).flex_shrink_0())
                    .child(div().flex_shrink_0().text_size(px(24.)).child(super::Text(
                        "<a href='https://example.com/offscreen'>Offscreen lyric</a>".into(),
                    )))
                    .child(div().h(px(400.)).flex_shrink_0())
            }
        }
        cx.use_platform_text_system();
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let probe = cx.new(|_| Probe {
                scroll: gpui::ScrollHandle::new(),
                reveals: 0,
                clicks: 0,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyric-links-reference.json")).unwrap();
        assert_eq!(reference["offscreen"]["scrolled"], true);
        assert_eq!(reference["offscreen"]["focused"], true);
        assert_eq!(reference["offscreen"]["visible"], true);
        probe.read_with(cx, |probe, _| assert_eq!(probe.scroll.offset().y, px(0.)));
        cx.update(|window, _| window.focus_next());
        cx.run_until_parked();
        probe.read_with(cx, |probe, _| assert!(probe.scroll.offset().y < px(0.)));
        probe.read_with(cx, |probe, _| assert_eq!(probe.reveals, 1));
        cx.update(|window, cx| {
            probe
                .read(cx)
                .scroll
                .set_offset(gpui::point(px(0.), px(0.)));
            window.refresh();
        });
        cx.run_until_parked();
        probe.read_with(cx, |probe, _| {
            assert_eq!(probe.scroll.offset().y, px(0.));
            assert_eq!(probe.reveals, 1);
        });
        cx.simulate_keystrokes("enter");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        for _ in 0..2 {
            cx.update(|window, cx| window.draw_next_frame_for_test(cx));
            cx.run_until_parked();
        }
        assert_eq!(
            cx.opened_url().as_deref(),
            reference["offscreen"]["url"].as_str()
        );
        probe.read_with(cx, |probe, _| assert_eq!(probe.reveals, 2));
        probe.read_with(cx, |probe, _| assert_eq!(probe.clicks, 2));
        cx.update(|window, cx| {
            probe
                .read(cx)
                .scroll
                .set_offset(gpui::point(px(0.), px(0.)));
            window.refresh();
        });
        cx.run_until_parked();
        let link = cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .find(|(_, node)| node.role() == gpui::accesskit::Role::Link)
                .unwrap()
                .0
        });
        cx.update(|window, cx| {
            window.dispatch_accessibility_action_for_test(
                gpui::accesskit::ActionRequest {
                    action: gpui::accesskit::Action::Click,
                    target_tree: gpui::accesskit::TreeId::ROOT,
                    target_node: link,
                    data: None,
                },
                cx,
            )
        });
        cx.run_until_parked();
        probe.read_with(cx, |probe, _| assert_eq!(probe.reveals, 3));
        for _ in 0..2 {
            cx.update(|window, cx| window.draw_next_frame_for_test(cx));
            cx.run_until_parked();
        }
        probe.read_with(cx, |probe, _| assert_eq!(probe.clicks, 3));
        assert_eq!(
            cx.opened_url().as_deref(),
            reference["offscreen"]["url"].as_str()
        );
    }
    #[gpui::test]
    fn ruby_line_spacing_stays_within_original_reference_bounds(cx: &mut TestAppContext) {
        struct Probe(serde_json::Value);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .w(px(self.0["width"].as_f64().unwrap() as f32))
                    .px(px(16.))
                    .text_size(px(self.0["size"].as_f64().unwrap() as f32))
                    .font_family("Inter")
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .line_height(relative(1.2))
                    .text_align(match self.0["alignment"].as_str().unwrap() {
                        "right" => gpui::TextAlign::Right,
                        "center" => gpui::TextAlign::Center,
                        _ => gpui::TextAlign::Left,
                    })
                    .child(div().debug_selector(|| "ruby-layout".into()).child(
                        if self.0["karaoke"] == true {
                            div()
                                .h_flex()
                                .child(crate::lyrics_highlight::Word {
                                    key: "karaoke-ruby-reference".into(),
                                    text: self.0["text"].as_str().unwrap().into(),
                                    position: 0.,
                                    start: 1.,
                                    duration: 4.,
                                    playing: false,
                                    base: gpui::white(),
                                    foreground: gpui::white(),
                                    zoom: 1.,
                                })
                                .into_any_element()
                        } else {
                            super::Text(self.0["text"].as_str().unwrap().into()).into_any_element()
                        },
                    ))
            }
        }
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "macos") {
            include_str!("../assets/lyrics-ruby-layout-darwin-reference.json")
        } else {
            include_str!("../assets/lyrics-ruby-layout-linux-reference.json")
        })
        .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let probe = cx.new(|_| Probe(reference["cases"][0].clone()));
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        let block_reference: serde_json::Value =
            serde_json::from_str(if cfg!(target_os = "macos") {
                include_str!("../assets/lyrics-block-layout-darwin-reference.json")
            } else {
                include_str!("../assets/lyrics-block-layout-linux-reference.json")
            })
            .unwrap();
        let light_reference: serde_json::Value =
            serde_json::from_str(if cfg!(target_os = "macos") {
                include_str!("../assets/lyrics-block-layout-darwin-light-reference.json")
            } else {
                include_str!("../assets/lyrics-block-layout-linux-light-reference.json")
            })
            .unwrap();
        let karaoke_reference: serde_json::Value =
            serde_json::from_str(if cfg!(target_os = "macos") {
                include_str!("../assets/lyrics-karaoke-layout-darwin-reference.json")
            } else {
                include_str!("../assets/lyrics-karaoke-layout-linux-reference.json")
            })
            .unwrap();
        let spacing_reference: serde_json::Value =
            serde_json::from_str(if cfg!(target_os = "macos") {
                include_str!("../assets/lyrics-spacing-layout-darwin-reference.json")
            } else {
                include_str!("../assets/lyrics-spacing-layout-linux-reference.json")
            })
            .unwrap();
        let overhang_reference: serde_json::Value =
            serde_json::from_str(if cfg!(target_os = "macos") {
                include_str!("../assets/lyrics-overhang-layout-darwin-reference.json")
            } else {
                include_str!("../assets/lyrics-overhang-layout-linux-reference.json")
            })
            .unwrap();
        for reference in [
            &reference,
            &block_reference,
            &light_reference,
            &karaoke_reference,
            &spacing_reference,
            &overhang_reference,
        ] {
            cx.update(|window, cx| {
                gpui_component::Theme::change(
                    if reference["theme"] == "light" {
                        gpui_component::ThemeMode::Light
                    } else {
                        gpui_component::ThemeMode::Dark
                    },
                    Some(window),
                    cx,
                );
            });
            for case in reference["cases"].as_array().unwrap() {
                cx.update(|window, cx| {
                    probe.update(cx, |probe, cx| {
                        probe.0 = case.clone();
                        cx.notify()
                    });
                    window.refresh();
                });
                cx.run_until_parked();
                let height = f64::from(cx.debug_bounds("ruby-layout").unwrap().size.height);
                if case["width"] == 600 && case["text"].as_str().unwrap().contains("long reading") {
                    let size = case["size"].as_f64().unwrap() as f32;
                    let mut actual = cx.update(|window, _| {
                        window
                            .painted_glyphs_for_test()
                            .iter()
                            .filter(|(bounds, _, _)| f32::from(bounds.size.height) > size * 0.7)
                            .map(|(bounds, _, _)| f64::from(bounds.center().x))
                            .collect::<Vec<_>>()
                    });
                    actual.sort_by(f64::total_cmp);
                    let mut expected =
                        if let Some(characters) = case["baseCharacters"].as_array() {
                            characters
                                .iter()
                                .map(|character| &character["bounds"])
                                .collect::<Vec<_>>()
                        } else {
                            case["rubies"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|ruby| &ruby["base"][0])
                                .collect()
                        }
                        .into_iter()
                        .map(|bounds| {
                            bounds[0].as_f64().unwrap() + bounds[2].as_f64().unwrap() / 2.
                        })
                        .collect::<Vec<_>>();
                    expected.sort_by(f64::total_cmp);
                    assert_eq!(actual.len(), expected.len());
                    for (actual, expected) in actual.iter().zip(expected) {
                        assert!(
                            (actual - expected).abs() <= 1.5,
                            "{case}: base glyph {actual}, original {expected}"
                        );
                    }
                }
                assert!(
                    // Whole wrapped blocks include native font and physical-pixel rounding.
                    (height - case["content"][3].as_f64().unwrap()).abs() <= 2.1,
                    "{case}: native height {height}"
                );
                for ruby in case["rubies"].as_array().unwrap() {
                    if case["karaoke"] == true {
                        assert_eq!(ruby["fontWeight"], "500");
                        let size = case["size"].as_f64().unwrap() * 0.55;
                        for key in ["fontSize", "lineHeight"] {
                            let value = ruby[key]
                                .as_str()
                                .unwrap()
                                .trim_end_matches("px")
                                .parse::<f64>()
                                .unwrap();
                            assert!((value - size).abs() < 1e-6);
                        }
                    } else {
                        assert_eq!(ruby["fontWeight"], "600");
                        assert_eq!(ruby["lineHeight"], "normal");
                        assert_eq!(
                            ruby["fontSize"],
                            format!("{}px", case["size"].as_f64().unwrap() * 0.5)
                        );
                    }
                }
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                if case["karaoke"] == true {
                    let count = case["readingText"].as_str().unwrap().chars().count();
                    if let Some(characters) = case["readingCharacters"].as_array() {
                        let first = &characters[0];
                        let last = characters.last().unwrap();
                        let expected = last[0].as_f64().unwrap() + last[1].as_f64().unwrap() / 2.
                            - first[0].as_f64().unwrap()
                            - first[1].as_f64().unwrap() / 2.;
                        let actual = cx.update(|window, _| {
                            let glyphs = window.painted_glyphs_for_test();
                            let centers = glyphs
                                .iter()
                                .take(count)
                                .map(|glyph| glyph.0.center().x)
                                .collect::<Vec<_>>();
                            f64::from(
                                *centers.iter().max().unwrap() - *centers.iter().min().unwrap(),
                            )
                        });

                        assert!(
                            (actual - expected).abs() <= 1.,
                            "{case}: reading separation {actual}, original {expected}"
                        );
                        continue;
                    }
                    let width = cx.update(|window, _| {
                        let glyphs = window.painted_glyphs_for_test();
                        let bounds = glyphs
                            .iter()
                            .take(count)
                            .map(|(bounds, _, _)| *bounds)
                            .reduce(|a, b| a.union(&b))
                            .unwrap();
                        let viewport = window.viewport_size();
                        let pixels = window.rasterized_scene_for_test(viewport).unwrap();
                        let scale = window.scale_factor();
                        let stride = (f32::from(viewport.width) * scale).round() as usize;
                        let left = (f32::from(bounds.left()).max(0.) * scale).floor() as usize;
                        let right = (f32::from(bounds.right()) * scale).ceil() as usize;
                        let top = (f32::from(bounds.top()).max(0.) * scale).floor() as usize;
                        let bottom = (f32::from(bounds.bottom().min(glyphs[count].0.top())) * scale)
                            .floor() as usize;
                        let mut ink = vec![];
                        for x in left..right {
                            if (top..bottom).any(|y| {
                                let pixel = &pixels[(y * stride + x) * 4..][..4];
                                pixel[..3].iter().all(|value| *value >= 128)
                            }) {
                                ink.push(x);
                            }
                        }
                        (ink.last().unwrap() - ink.first().unwrap() + 1) as f32 / scale
                    });
                    assert!(
                        (f64::from(width) - case["readingInkWidth"].as_f64().unwrap()).abs() <= 1.5,
                        "{case}: native reading ink width {width:?}"
                    );
                }
                for link in case["links"].as_array().unwrap() {
                    assert_eq!(link["decoration"], "none");
                    let color =
                        feishin_core::appearance::color(link["color"].as_str().unwrap()).unwrap();
                    let color: gpui::Hsla = gpui::Rgba {
                        r: color[0],
                        g: color[1],
                        b: color[2],
                        a: color[3],
                    }
                    .into();
                    let count = cx.update(|window, _| {
                        window
                            .painted_glyphs_for_test()
                            .iter()
                            .filter(|(_, _, actual)| *actual == color)
                            .count()
                    });
                    assert!(
                        count >= link["text"].as_str().unwrap().chars().count(),
                        "{}: {case}, matching glyphs={count}",
                        reference["theme"]
                    );
                }
            }
        }
    }
    #[gpui::test]
    fn lyric_links_support_tab_enter_and_accessibility_without_losing_seek_clicks(
        cx: &mut TestAppContext,
    ) {
        struct Probe {
            alignment: gpui::TextAlign,
            width: f32,
            scale: f32,
            clicks: usize,
            formatted: bool,
        }
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .id("lyric-link-probe")
                    .w(px(self.width))
                    .text_size(px(24.))
                    .line_height(relative(1.2))
                    .font_family("Inter")
                    .text_align(self.alignment)
                    .paint_scale(self.scale)
                    .paint_scale_origin(gpui::point(0., 0.5))
                    .on_click(cx.listener(|this, _, _, _| this.clicks += 1))
                    .child({
                        let text = "prefix <a href='https://example.com/first'>A long link that wraps onto several rows</a> middle <a href='https://example.com/second'>العربية</a> <a href='javascript:bad()'>unsafe</a>";
                        if self.formatted {
                            gpui_component::text::TextView::html("formatted-links", text, window, cx)
                                .style(gpui_component::text::TextViewStyle { safe_urls_only: true, ..Default::default() })
                                .selectable(true)
                                .into_any_element()
                        } else {
                            super::Text(text.into()).into_any_element()
                        }
                    })
            }
        }
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let probe = cx.new(|_| Probe {
                alignment: gpui::TextAlign::Left,
                width: 160.,
                scale: 0.95,
                clicks: 0,
                formatted: false,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyric-links-reference.json")).unwrap();
        for alignment in [
            gpui::TextAlign::Left,
            gpui::TextAlign::Center,
            gpui::TextAlign::Right,
        ] {
            for width in [160., 500.] {
                for scale in [0.95, 1.] {
                    for formatted in [false, true] {
                        let original = reference["cases"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|case| {
                                case["alignment"]
                                    == match alignment {
                                        gpui::TextAlign::Left => "left",
                                        gpui::TextAlign::Center => "center",
                                        _ => "right",
                                    }
                                    && case["width"].as_f64() == Some(width as f64)
                                    && (case["scale"].as_f64().unwrap() - scale as f64).abs() < 1e-6
                            })
                            .unwrap();
                        cx.update(|window, cx| {
                            probe.update(cx, |probe, cx| {
                                probe.alignment = alignment;
                                probe.width = width;
                                probe.scale = scale;
                                probe.clicks = 0;
                                probe.formatted = formatted;
                                cx.notify();
                            });
                            window.blur();
                            window.refresh();
                        });
                        cx.run_until_parked();
                        if formatted {
                            std::thread::sleep(std::time::Duration::from_millis(250));
                            cx.run_until_parked();
                        }
                        let tree = cx.update(|window, _| {
                            window.accessibility_tree_for_test().unwrap().clone()
                        });
                        accesskit_consumer::Tree::new(tree.clone(), true);
                        let links = tree
                            .nodes
                            .iter()
                            .filter(|(_, node)| node.role() == gpui::accesskit::Role::Link)
                            .collect::<Vec<_>>();
                        assert_eq!(links.len(), 2);
                        assert_eq!(
                            links[0].1.label(),
                            Some("A long link that wraps onto several rows")
                        );
                        assert_eq!(links[1].1.label(), Some("العربية"));
                        assert!(
                            links
                                .iter()
                                .all(|(_, node)| node
                                    .supports_action(gpui::accesskit::Action::Click))
                        );
                        let ids = [links[0].0, links[1].0];
                        for (index, url) in
                            ["https://example.com/first", "https://example.com/second"]
                                .iter()
                                .enumerate()
                        {
                            assert_eq!(original["traces"][index]["url"].as_str(), Some(*url));
                            assert_eq!(
                                original["traces"][index]["seeks"].as_array().unwrap().len(),
                                index + 1
                            );
                            cx.update(|window, _| window.focus_next());
                            cx.run_until_parked();
                            let focus = cx.update(|window, _| {
                                window.accessibility_tree_for_test().unwrap().focus
                            });
                            assert_eq!(focus, ids[index], "{alignment:?} {width} {scale}");
                            cx.simulate_keystrokes("space");
                            cx.run_until_parked();
                            probe.read_with(cx, |probe, _| {
                                assert_eq!(probe.clicks, if formatted { 0 } else { index })
                            });
                            cx.simulate_keystrokes("enter");
                            cx.run_until_parked();
                            assert_eq!(
                                cx.opened_url().as_deref(),
                                Some(*url),
                                "{alignment:?} {width} {scale}"
                            );
                            probe.read_with(cx, |probe, _| {
                                assert_eq!(probe.clicks, if formatted { 0 } else { index + 1 })
                            });
                        }
                        cx.update(|window, cx| {
                            window.dispatch_accessibility_action_for_test(
                                gpui::accesskit::ActionRequest {
                                    action: gpui::accesskit::Action::Click,
                                    target_tree: gpui::accesskit::TreeId::ROOT,
                                    target_node: ids[0],
                                    data: None,
                                },
                                cx,
                            )
                        });
                        cx.run_until_parked();
                        assert_eq!(
                            cx.opened_url().as_deref(),
                            Some("https://example.com/first")
                        );
                        probe.read_with(cx, |probe, _| {
                            assert_eq!(probe.clicks, if formatted { 0 } else { 3 })
                        });
                    }
                }
            }
        }
    }
    #[gpui::test]
    fn lyric_links_follow_scaled_text_and_do_not_open_unsafe_urls(cx: &mut TestAppContext) {
        struct Probe {
            unsafe_url: bool,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let href = if self.unsafe_url {
                    "javascript:alert(1)"
                } else {
                    "https://example.com/music"
                };
                div()
                    .w(px(1000.))
                    .text_size(px(24.))
                    .line_height(relative(1.2))
                    .font_family("Inter")
                    .paint_scale(0.95)
                    .paint_scale_origin(gpui::point(0., 0.5))
                    .child(super::Text(format!(
                        "{} <a href='{href}'>Link</a>",
                        "A".repeat(40)
                    )))
            }
        }
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let probe = cx.new(|_| Probe { unsafe_url: false });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let glyph = cx.update(|window, _| window.painted_glyphs_for_test()[41].0);
        cx.simulate_click(glyph.center(), gpui::Modifiers::none());
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://example.com/music")
        );
        cx.update(|window, cx| {
            probe.update(cx, |probe, cx| {
                probe.unsafe_url = true;
                cx.notify()
            });
            window.refresh()
        });
        cx.run_until_parked();
        cx.simulate_click(glyph.center(), gpui::Modifiers::none());
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://example.com/music")
        );
    }
    #[gpui::test]
    fn formatted_word_twins_preserve_readings_and_shape_during_highlighting(
        cx: &mut TestAppContext,
    ) {
        struct Probe {
            position: f64,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let mut base = cx.theme().foreground;
                base.a = 0.2;
                div()
                    .flex()
                    .w(px(240.))
                    .text_size(px(24.))
                    .line_height(relative(1.2))
                    .font_family("Inter")
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(crate::lyrics_highlight::Word {
                        key: "markup".into(),
                        text: "<b>A &amp; B</b> <ruby>君<rt>きみ</rt><rp>(fallback)</rp></ruby>"
                            .into(),
                        position: self.position,
                        start: 1.,
                        duration: 2.,
                        playing: true,
                        base,
                        foreground: cx.theme().foreground,
                        zoom: 1.,
                    })
            }
        }
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let probe = cx.new(|_| Probe { position: 1.5 });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        let mut previous = None;
        for position in [1.5, 2., 2.5] {
            cx.update(|window, cx| {
                probe.update(cx, |probe, cx| {
                    probe.position = position;
                    cx.notify()
                });
                window.refresh()
            });
            cx.run_until_parked();
            let bounds = cx.debug_bounds("lyric-word-progress-markup").unwrap();
            assert!(
                bounds.size.height > px(24. * 1.2),
                "Ruby reading is missing: {bounds:?}"
            );
            let (tree, glyphs) = cx.update(|window, _| {
                (
                    window.accessibility_tree_for_test().unwrap().clone(),
                    window.painted_glyphs_for_test(),
                )
            });
            let text = tree
                .nodes
                .iter()
                .filter_map(|(_, node)| node.value())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                text.contains("A & B") && text.contains("君") && text.contains("きみ"),
                "{text:?}"
            );
            assert!(!text.contains('<') && !text.contains("fallback"));
            let base = glyphs
                .iter()
                .filter(|(_, _, color)| color.a == 0.2)
                .map(|(bounds, _, _)| *bounds)
                .collect::<Vec<_>>();
            assert!(!base.is_empty());
            for (glyph, _, color) in glyphs.iter().filter(|(_, _, color)| color.a > 0.2) {
                assert!(
                    base.contains(glyph),
                    "Styled highlight changed glyph geometry: {glyph:?}, {color:?}"
                );
            }
            if let Some(previous) = &previous {
                assert_eq!(&base, previous)
            } else {
                previous = Some(base)
            }
        }
    }
}
