use gpui::{prelude::*, *};
use gpui_component::animation::CssTransition;
use std::time::Duration;

struct State {
    opacity: CssTransition,
    scale: CssTransition,
    hovered: bool,
}
#[derive(IntoElement)]
pub(crate) struct Row {
    pub index: usize,
    pub information: bool,
    pub sidebar: bool,
    pub synchronized: bool,
    pub karaoke: bool,
    pub bright: bool,
    pub font_size: f32,
    pub rem: f32,
    pub gap_after: f32,
    pub padding_left: f32,
    pub padding_right: f32,
    pub opacity: f32,
    pub scale: f32,
    pub alignment: String,
    pub content: AnyElement,
    pub click: Option<Click>,
}
type Click = std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub(crate) fn is_bright(
    lines: &[feishin_core::lyrics::Line],
    index: usize,
    position: f64,
    playing: bool,
    karaoke: bool,
    lead: f64,
) -> bool {
    let any_words = lines.iter().any(|line| {
        !line.words.is_empty() || line.cue_lines.iter().any(|cue| !cue.words.is_empty())
    });
    if position == 0. && !playing
        || !any_words && lines.iter().all(|line| line.time.unwrap_or_default() == 0.)
    {
        return false;
    }
    let line = &lines[index];
    let time = line.time.unwrap_or_default();
    let next = lines
        .get(index + 1)
        .and_then(|line| line.time)
        .unwrap_or(f64::INFINITY);
    let end = line
        .cue_lines
        .iter()
        .filter_map(|cue| cue.end)
        .chain(
            line.cue_lines
                .iter()
                .flat_map(|cue| cue.words.iter().filter_map(|word| word.end)),
        )
        .chain(line.words.iter().filter_map(|word| word.end))
        .fold(time, f64::max);
    let in_window = position < next || position < end;
    if !karaoke {
        return position >= time && in_window;
    }
    if !playing {
        return false;
    }
    let has_words =
        !line.words.is_empty() || line.cue_lines.iter().any(|cue| !cue.words.is_empty());
    let interpolated = position + if any_words { 0.15 } else { 0.115 };
    position >= time - lead && in_window
        || !has_words && interpolated + 2. >= time && interpolated < next.max(end + 0.05)
}
impl RenderOnce for Row {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let now = cx.background_executor().now();
        let state = window.use_keyed_state(
            SharedString::from(format!(
                "lyric-row-motion-{}-{}",
                self.information, self.index
            )),
            cx,
            |_, _| State {
                opacity: CssTransition::new(
                    if self.synchronized { self.opacity } else { 1. },
                    Duration::from_millis(600),
                    [0.16, 1., 0.3, 1.],
                ),
                scale: CssTransition::new(
                    if self.synchronized { self.scale } else { 1. },
                    Duration::from_millis(600),
                    [0.16, 1., 0.3, 1.],
                ),
                hovered: false,
            },
        );
        let (opacity, scale, animating) = state.update(cx, |state, _| {
            state.opacity.retarget(
                if !self.synchronized || self.bright || state.hovered {
                    1.
                } else {
                    self.opacity
                },
                now,
            );
            state.scale.retarget(
                if !self.synchronized || self.bright {
                    1.
                } else {
                    self.scale
                },
                now,
            );
            let (opacity, fading) = state.opacity.value(now);
            let (scale, scaling) = state.scale.value(now);
            (opacity, scale, fading || scaling)
        });
        if animating {
            window.request_animation_frame();
        }
        let origin = match self.alignment.as_str() {
            "right" => 1.,
            "center" => 0.5,
            _ => 0.,
        };
        let index = self.index;
        let sidebar = self.sidebar;
        let click = self.click;
        div()
            .id((
                if self.information {
                    "lyric-information"
                } else {
                    "lyric"
                },
                index,
            ))
            .debug_selector(move || {
                format!(
                    "lyric-line-{}-{index}",
                    if sidebar { "sidebar" } else { "player" }
                )
            })
            .w(relative(
                (1. - (self.padding_left + self.padding_right) / 100.).max(0.),
            ))
            .ml(relative(self.padding_left / 100.))
            .min_w_0()
            .flex_shrink_0()
            .mb(px(self.gap_after))
            .text_size(px(self.font_size))
            .line_height(relative(1.2))
            .font_weight(FontWeight::SEMIBOLD)
            .text_align(match self.alignment.as_str() {
                "right" => TextAlign::Right,
                "center" => TextAlign::Center,
                _ => TextAlign::Left,
            })
            .text_wrap(TextWrap::BreakWord)
            .px(px(self.rem))
            .when(
                self.sidebar && self.synchronized && !self.karaoke && !self.information,
                |row| row.py(px(self.rem * 0.25)),
            )
            .opacity(opacity)
            .paint_scale(scale)
            .paint_scale_origin(point(origin, 0.5))
            .when(!self.information, |row| {
                row.on_hover(move |hovered, _, cx| {
                    state.update(cx, |state, cx| {
                        if state.hovered != *hovered {
                            state.hovered = *hovered;
                            cx.notify();
                        }
                    })
                })
            })
            .when(self.synchronized && !self.information, |row| {
                row.cursor_pointer()
            })
            .when_some(click, |row, click| {
                row.on_click(move |event, window, cx| click(event, window, cx))
            })
            .child(self.content)
    }
}

pub(crate) struct Gutter {
    signature: u64,
    size: Size<Pixels>,
    enabled: bool,
}
pub(crate) fn gutter_signature(
    app: &crate::app::Feishin,
    key: &str,
    information: &[String],
    window: &Window,
    cx: &App,
) -> u64 {
    use gpui_component::ActiveTheme;
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    information.hash(&mut hash);
    (
        app.lyric_generation,
        app.annotation_generation,
        app.lyric_translation_generation,
    )
        .hash(&mut hash);
    for line in &app.lyrics {
        line.text.hash(&mut hash);
        line.time.is_some().hash(&mut hash);
        for word in &line.words {
            word.text.hash(&mut hash);
        }
        for cue in &line.cue_lines {
            cue.text.hash(&mut hash);
            for word in &cue.words {
                word.text.hash(&mut hash);
            }
        }
    }
    cx.theme().font_family.hash(&mut hash);
    app.state.settings.lyric_overlay_layers.hash(&mut hash);
    app.lyric_translated.hash(&mut hash);
    (
        app.lyric_annotations.len(),
        app.state.settings.lyric_furigana,
        app.state.settings.lyric_romaji,
        app.lyric_translation_visible,
    )
        .hash(&mut hash);
    let display = app.state.settings.lyric_display_for(key);
    for value in [
        app.state.settings.zoom,
        f32::from(cx.theme().font_size),
        f32::from(window.viewport_size().width),
        f32::from(window.viewport_size().height),
        display.font_size,
        display.font_size_unsync,
        display.gap,
        display.gap_unsync,
        display.padding_left,
        display.padding_right,
    ] {
        value.to_bits().hash(&mut hash);
    }
    hash.finish()
}
pub(crate) fn gutter_state(
    signature: u64,
    sidebar: bool,
    window: &mut Window,
    cx: &mut App,
) -> (bool, Entity<Gutter>) {
    let state = window.use_keyed_state(
        SharedString::from(format!("lyric-gutter-{sidebar}")),
        cx,
        |_, _| Gutter {
            signature,
            size: Size::default(),
            enabled: false,
        },
    );
    let enabled = state.update(cx, |state, _| {
        if state.signature != signature {
            state.signature = signature;
            state.enabled = false;
        }
        state.enabled
    });
    (enabled, state)
}
pub(crate) fn gutter_monitor(scroll: &ScrollHandle, state: Entity<Gutter>) -> impl IntoElement {
    let scroll = scroll.clone();
    canvas(
        move |_, window, cx| {
            let refresh = state.update(cx, |state, _| {
                let size = scroll.bounds().size;
                let resized = state.size != size;
                let enabled = !resized && scroll.max_offset().height > px(0.01);
                let changed = resized || state.enabled != enabled;
                state.size = size;
                state.enabled = enabled;
                changed
            });
            if refresh {
                window.defer(cx, |window, _| window.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use crate::app::Feishin;
    use feishin_core::{
        lyrics::{CueLine, Line, Word},
        storage::State,
    };
    use gpui::{
        AppContext, Context, Entity, IntoElement, Render, TestAppContext, Window, div, prelude::*,
        px, size,
    };
    use gpui_component::{ActiveTheme, Root, Theme};
    use std::{cell::RefCell, rc::Rc, time::Duration};

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "freebsd"))]
    #[gpui::test]
    fn lyric_edge_mask_matches_original_composited_pixels(cx: &mut TestAppContext) {
        struct MaskProbe(serde_json::Value);
        impl Render for MaskProbe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().size_full().bg(gpui::rgb(0x123456)).child(
                    div()
                        .size_full()
                        .opacity(self.0["opacity"].as_f64().unwrap() as f32)
                        .child(
                            div()
                                .absolute()
                                .left(px(10.))
                                .top(px(self.0["top"].as_f64().unwrap() as f32))
                                .w(px(100.))
                                .h(px(self.0["height"].as_f64().unwrap() as f32))
                                .bg(gpui::white())
                                .paint_scale(self.0["scale"].as_f64().unwrap() as f32)
                                .paint_scale_origin(gpui::point(0., 0.5))
                                .vertical_alpha_mask(0.15)
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .top(px(80.))
                                        .w(px(50.))
                                        .h(px(40.))
                                        .bg(gpui::rgb(0xf04173)),
                                ),
                        ),
                )
            }
        }
        #[cfg(target_os = "macos")]
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyrics-mask-darwin-reference.json"))
                .unwrap();
        #[cfg(not(target_os = "macos"))]
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyrics-mask-linux-reference.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let (_, cx) = cx.add_window_view(|_, _| MaskProbe(case.clone()));
            cx.run_until_parked();
            let pixels = cx.update(|window, _| {
                window
                    .rasterized_scene_for_test(size(px(128.), px(256.)))
                    .unwrap()
            });
            let image = image::open(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .join(case["file"].as_str().unwrap()),
            )
            .unwrap()
            .into_rgba8();
            assert_eq!(image.dimensions(), (256, 512));
            assert_eq!(pixels.len(), image.as_raw().len());
            let mean = pixels
                .iter()
                .zip(image.as_raw())
                .map(|(a, b)| f64::from(a.abs_diff(*b)))
                .sum::<f64>()
                / pixels.len() as f64;
            assert!(
                mean < 0.8,
                "Alpha-mask pixel difference: {case}, mean {mean}"
            );
            for y in [45, 55, 65, 75, 90, 120, 200, 310, 390, 410, 425, 435] {
                let offset = (y * 256 + 100) * 4;
                for (a, b) in pixels[offset..offset + 4]
                    .iter()
                    .zip(&image.as_raw()[offset..offset + 4])
                {
                    assert!(
                        a.abs_diff(*b) <= 3,
                        "Mask ramp differs at {y}: {case}, actual {a}, original {b}"
                    );
                }
            }
        }
    }

    #[test]
    fn line_brightness_matches_original_engine() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyrics-line-reference.json")).unwrap();
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 1080);
        for case in fixture["cases"].as_array().unwrap() {
            let lines: Vec<Line> = case["lyrics"]
                .as_array()
                .unwrap()
                .iter()
                .map(|line| Line {
                    time: Some(line["startMs"].as_f64().unwrap() / 1000.),
                    text: "Fixture".into(),
                    words: vec![],
                    cue_lines: line["cueLines"]
                        .as_array()
                        .map(|cues| {
                            cues.iter()
                                .map(|cue| CueLine {
                                    start: cue["startMs"].as_f64().unwrap() / 1000.,
                                    end: Some(cue["endMs"].as_f64().unwrap() / 1000.),
                                    text: "Fixture".into(),
                                    agent_id: None,
                                    words: cue["words"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .map(|word| Word {
                                            time: word["startMs"].as_f64().unwrap() / 1000.,
                                            end: Some(word["endMs"].as_f64().unwrap() / 1000.),
                                            text: "Fixture".into(),
                                        })
                                        .collect(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect();
            for (index, row) in case["rows"].as_array().unwrap().iter().enumerate() {
                assert_eq!(
                    super::is_bright(
                        &lines,
                        index,
                        case["position"].as_f64().unwrap() / 1000.,
                        case["playing"].as_bool().unwrap(),
                        case["prefix"] == "karaoke-line",
                        case["lead"].as_f64().unwrap() / 1000.
                    ),
                    row["bright"].as_bool().unwrap(),
                    "{case}: row {index}"
                );
            }
        }
    }

    #[gpui::test]
    fn lyric_rows_match_original_layout_and_scale_origins(cx: &mut TestAppContext) {
        struct Probe {
            app: Entity<Feishin>,
            sidebar: bool,
            root_size: Option<gpui::Pixels>,
        }
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                // Keep the fixture's root override when the app's periodic theme scan runs.
                if let Some(size) = self.root_size {
                    Theme::global_mut(cx).font_size = size;
                    window.set_rem_size(size);
                }
                let rem = f32::from(cx.theme().font_size);
                let key = if self.sidebar { "sidebar" } else { "default" };
                let content = self
                    .app
                    .update(cx, |app, cx| app.lyrics_surface_for(key, window, cx));
                div()
                    .w(px(700.))
                    .h(px(600.))
                    .font(crate::fonts::app_font(cx))
                    .text_color(cx.theme().foreground)
                    .when(self.sidebar, |div| div.p(px(rem * 0.75)))
                    .child(content)
            }
        }
        cx.set_assets(crate::icons::Assets);
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
        let dir =
            std::env::temp_dir().join(format!("feishin-lyric-layout-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(State::default()), window, cx));
            let probe = cx.new(|_| Probe {
                app,
                sidebar: false,
                root_size: None,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        #[cfg(target_os = "macos")]
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/lyrics-layout-darwin-reference.json"
        ))
        .unwrap();
        #[cfg(not(target_os = "macos"))]
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyrics-layout-linux-reference.json"))
                .unwrap();
        assert_eq!(fixture["cases"].as_array().unwrap().len(), 288);
        let information: serde_json::Value = serde_json::from_str(if cfg!(target_os = "macos") {
            include_str!("../assets/lyrics-information-layout-darwin-reference.json")
        } else {
            include_str!("../assets/lyrics-information-layout-linux-reference.json")
        })
        .unwrap();
        assert_eq!(information["cases"].as_array().unwrap().len(), 864);
        for case in fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .chain(information["cases"].as_array().unwrap())
        {
            let root = case["rootSize"].as_f64().unwrap() as f32;
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let sidebar = case["sidebar"].as_bool().unwrap();
            let kind = case["kind"].as_str().unwrap();
            let active = case["active"].as_bool().unwrap();
            cx.simulate_resize(size(
                px(case["viewport"][0].as_f64().unwrap() as f32),
                px(case["viewport"][1].as_f64().unwrap() as f32),
            ));
            cx.update(|window, cx| {
                Theme::global_mut(cx).font_size = px(root * zoom);
                Theme::global_mut(cx).foreground = gpui::rgb(0xffffff).into();
                Theme::global_mut(cx).font_family = "Inter".into();
                window.set_rem_size(px(root * zoom));
                window.text_system().set_optical_size_scale(zoom);
                probe.update(cx, |probe, cx| {
                    probe.sidebar = sidebar;
                    probe.root_size = Some(px(root * zoom));
                    probe.app.update(cx, |app, cx| {
                        app.state.settings.zoom = zoom;
                        app.state.settings.lyric_alignment =
                            case["alignment"].as_str().unwrap().into();
                        app.state.settings.lyric_words = kind == "karaoke";
                        app.state.settings.lyric_show_provider =
                            case["information"]["provider"] == true;
                        app.state.settings.lyric_show_match = case["information"]["match"] == true;
                        app.lyric_record =
                            case.get("information")
                                .map(|_| feishin_core::lyric_sources::Record {
                                    raw: String::new(),
                                    selected: 0,
                                    variants: vec![],
                                    matched: Some(feishin_core::lyric_sources::Match {
                                        provider: feishin_core::lyric_sources::Provider::Lrclib,
                                        id: "fixture".into(),
                                        name: "Fixture".into(),
                                        artist: "Artist".into(),
                                        synchronized: kind != "plain",
                                        score: 0.,
                                    }),
                                });
                        app.state.settings.lyric_offset = 0.;
                        app.state.settings.lyric_display.padding_left = 10.;
                        app.state.settings.lyric_display.padding_right = 5.;
                        app.state.settings.lyric_displays.clear();
                        app.state.queue.position = if active { 5.2 } else { 0. };
                        app.playing = active;
                        app.loaded = false;
                        app.lyrics = case["rows"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .enumerate()
                            .map(|(index, _row)| {
                                let start = 1. + index as f64 * 2.;
                                let text = case["texts"][index].as_str().unwrap().to_string();
                                Line {
                                    time: (kind != "plain").then_some(start),
                                    text: text.clone(),
                                    words: vec![],
                                    cue_lines: if kind == "karaoke" {
                                        vec![CueLine {
                                            start,
                                            end: Some(start + 2.),
                                            text: text.clone(),
                                            agent_id: None,
                                            words: vec![Word {
                                                time: start,
                                                end: Some(start + 2.),
                                                text,
                                            }],
                                        }]
                                    } else {
                                        vec![]
                                    },
                                }
                            })
                            .collect();
                        cx.notify();
                    });
                    cx.notify();
                });
                window.refresh();
            });
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(600));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            let (tree, dpr) = cx.update(|window, _| {
                (
                    window.accessibility_tree_for_test().unwrap().clone(),
                    f64::from(window.scale_factor()),
                )
            });
            let scroll_stats = probe.read_with(cx, |probe, cx| {
                probe.app.read_with(cx, |app, _| {
                    let scroll = if sidebar {
                        &app.sidebar_lyric_scroll
                    } else {
                        &app.lyric_scroll
                    };
                    (
                        scroll.bounds(),
                        scroll.max_offset(),
                        (0..scroll.children_count())
                            .map(|index| scroll.bounds_for_item(index))
                            .collect::<Vec<_>>(),
                    )
                })
            });
            for (index, expected) in case["rows"].as_array().unwrap().iter().enumerate() {
                let bounds = cx
                    .debug_bounds(&format!(
                        "lyric-line-{}-{index}",
                        if sidebar { "sidebar" } else { "player" }
                    ))
                    .unwrap();
                let scale = if kind != "plain" && !(active && index == 2) {
                    0.95
                } else {
                    1.
                };
                let origin = match case["alignment"].as_str().unwrap() {
                    "center" => 0.5,
                    "right" => 1.,
                    _ => 0.,
                };
                let actual = [
                    f32::from(bounds.origin.x)
                        + f32::from(bounds.size.width) * origin * (1. - scale),
                    f32::from(bounds.origin.y) + f32::from(bounds.size.height) * 0.5 * (1. - scale),
                    f32::from(bounds.size.width) * scale,
                    f32::from(bounds.size.height) * scale,
                ];
                for (value, expected) in actual
                    .into_iter()
                    .zip(expected["bounds"].as_array().unwrap())
                {
                    assert!(
                        (f64::from(value) - expected.as_f64().unwrap()).abs() <= 1.1,
                        "{case}: row {index}: {bounds:?}, transformed {actual:?}, scroll {scroll_stats:?}"
                    );
                }
                assert!(
                    tree.nodes
                        .iter()
                        .filter(|(_, node)| node.role() == gpui::accesskit::Role::Button)
                        .filter_map(|(_, node)| node.bounds())
                        .any(|rect| {
                            [
                                rect.x0 / dpr,
                                rect.y0 / dpr,
                                (rect.x1 - rect.x0) / dpr,
                                (rect.y1 - rect.y0) / dpr,
                            ]
                            .into_iter()
                            .zip(actual)
                            .all(|(value, expected)| (value - f64::from(expected)).abs() < 0.01)
                        }),
                    "Scaled accessible hit area differs: {case}, {actual:?}"
                );
            }
            if let Some(rows) = case["informationRows"].as_array() {
                for (index, row) in rows.iter().enumerate() {
                    let bounds = cx
                        .debug_bounds(&format!(
                            "lyric-line-{}-{}",
                            if sidebar { "sidebar" } else { "player" },
                            3 + index
                        ))
                        .unwrap();
                    let scale = if kind == "plain" { 1. } else { 0.95 };
                    let origin = match case["alignment"].as_str().unwrap() {
                        "center" => 0.5,
                        "right" => 1.,
                        _ => 0.,
                    };
                    let actual = [
                        f32::from(bounds.origin.x)
                            + f32::from(bounds.size.width) * origin * (1. - scale),
                        f32::from(bounds.origin.y)
                            + f32::from(bounds.size.height) * 0.5 * (1. - scale),
                        f32::from(bounds.size.width) * scale,
                        f32::from(bounds.size.height) * scale,
                    ];
                    for (value, expected) in
                        actual.into_iter().zip(row["bounds"].as_array().unwrap())
                    {
                        assert!(
                            (f64::from(value) - expected.as_f64().unwrap()).abs() <= 1.1,
                            "{case}: information {index}, {actual:?}"
                        );
                    }
                    assert!(
                        tree.nodes
                            .iter()
                            .filter_map(|(_, node)| node.value())
                            .any(|text| text == row["text"].as_str().unwrap()),
                        "Information missing from rendered text: {row}"
                    );
                }
            }
            let expected_max = (case["content"][1].as_f64().unwrap()
                + case["content"][3].as_f64().unwrap()
                + case["containerPadding"][1]
                    .as_str()
                    .unwrap()
                    .trim_end_matches("px")
                    .parse::<f64>()
                    .unwrap()
                    * f64::from(zoom)
                - case["container"][1].as_f64().unwrap()
                - case["container"][3].as_f64().unwrap())
            .max(0.);
            let actual_max = probe.read_with(cx, |probe, cx| {
                probe.app.read_with(cx, |app, _| {
                    f64::from(
                        if sidebar {
                            &app.sidebar_lyric_scroll
                        } else {
                            &app.lyric_scroll
                        }
                        .max_offset()
                        .height,
                    )
                })
            });
            assert!(
                (actual_max - expected_max).abs() <= 2.,
                "Scroll limit differs: {case}, {actual_max}, {expected_max}"
            );
        }
        if dir.exists() {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
