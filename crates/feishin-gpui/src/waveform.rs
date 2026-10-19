use crate::app::Feishin;
use feishin_core::{
    model::ItemKind,
    waveform::{Config, Data},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Selectable, StyledExt,
    slider::{SliderEvent, SliderState},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct Job(pub Arc<AtomicBool>);
impl Drop for Job {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
impl Feishin {
    pub(crate) fn init_waveform_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (index, value) in [
            self.state.settings.playerbar.bar_width,
            self.state.settings.playerbar.bar_gap,
            self.state.settings.playerbar.bar_radius,
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(slider) = self.waveform_sliders.get(index) {
                slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
                continue;
            }
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(if index == 2 { 20. } else { 10. })
                    .step(1.)
                    .default_value(value)
            });
            self.subscriptions.push(cx.subscribe(
                &slider,
                move |this, _, event: &SliderEvent, cx| {
                    if let SliderEvent::ChangeEnd(value) = event {
                        let config = &mut this.state.settings.playerbar;
                        let field = match index {
                            0 => &mut config.bar_width,
                            1 => &mut config.bar_gap,
                            _ => &mut config.bar_radius,
                        };
                        *field = value.end();
                        this.save();
                        cx.notify();
                    }
                },
            ));
            self.waveform_sliders.push(slider);
        }
    }

    pub(crate) fn waveform_controls(&self, query: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let matches = |key: &str| {
            feishin_core::settings_search::matches(&self.state.settings.language, query, &[key])
        };
        let row = |key: &str| {
            div().v_flex().gap_2().child(self.tr(key)).child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.tr(&format!("{key}_description"))),
            )
        };
        let config = &self.state.settings.playerbar;
        let mut rows = Vec::new();
        for (key, choices, selected) in [
            (
                "setting.playerbarSlider",
                vec![
                    ("slider", "setting.playerbarSliderType_optionSlider"),
                    ("waveform", "setting.playerbarSliderType_optionWaveform"),
                ],
                config.kind.as_str(),
            ),
            (
                "setting.playerbarWaveformAlign",
                vec![
                    ("top", "setting.playerbarWaveformAlign_optionTop"),
                    ("center", "setting.playerbarWaveformAlign_optionCenter"),
                    ("bottom", "setting.playerbarWaveformAlign_optionBottom"),
                ],
                config.bar_align.as_str(),
            ),
        ] {
            if !matches(key)
                || (key == "setting.playerbarWaveformAlign" && config.kind != "waveform")
            {
                continue;
            }
            let mut choices_view = div()
                .h_flex()
                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                .gap_1();
            for (value, label) in choices {
                choices_view = choices_view.child(
                    crate::styles::button(SharedString::from(format!("{key}-{value}")), cx)
                        .label(self.tr(label))
                        .selected(selected == value)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if key == "setting.playerbarSlider" {
                                this.state.settings.playerbar.kind = value.into();
                                this.reset_waveform();
                            } else {
                                this.state.settings.playerbar.bar_align = value.into();
                            }
                            this.save();
                            cx.notify();
                        })),
                );
            }
            rows.push(row(key).child(choices_view).into_any_element());
        }
        if config.kind == "waveform" {
            for (index, key) in [
                "setting.playerbarWaveformBarWidth",
                "setting.playerbarWaveformGap",
                "setting.playerbarWaveformRadius",
            ]
            .into_iter()
            .enumerate()
            {
                if matches(key) {
                    rows.push(
                        row(key)
                            .child(
                                div()
                                    .w(px(120.))
                                    .debug_selector(move || format!("waveform-setting-{index}"))
                                    .child(
                                        crate::styles::slider(&self.waveform_sliders[index], cx)
                                            .aria_label(self.tr(key)),
                                    ),
                            )
                            .into_any_element(),
                    );
                }
            }
            if matches("setting.playerbarWaveformStretch") {
                rows.push(
                    self.setting_checkbox(
                        "waveform-stretch",
                        "setting.playerbarWaveformStretch",
                        config.stretched,
                        cx,
                    )
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.state.settings.playerbar.stretched = *checked;
                        this.save();
                        cx.notify();
                    }))
                    .into_any_element(),
                );
            }
            if matches("setting.waveformLoadingDelay") {
                rows.push(
                    row("setting.waveformLoadingDelay")
                        .child(crate::styles::number_input(&self.player_numbers[4], cx).w(px(100.)))
                        .into_any_element(),
                );
            }
        }
        if matches("setting.showSkipButtons") {
            rows.push(
                self.setting_checkbox(
                    "waveform-skip-buttons",
                    "setting.showSkipButtons",
                    config.skip_enabled,
                    cx,
                )
                .on_click(cx.listener(|this, checked: &bool, _, cx| {
                    this.state.settings.playerbar.skip_enabled = *checked;
                    this.save();
                    cx.notify();
                }))
                .into_any_element(),
            );
        }
        if matches("setting.skipDuration") {
            rows.push(
                row("setting.skipDuration")
                    .child(
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_3()
                            .children(
                                [("common.backward", 5), ("common.forward", 6)]
                                    .into_iter()
                                    .map(|(key, index)| {
                                        div().v_flex().gap_1().child(self.tr(key)).child(
                                            crate::styles::number_input(
                                                &self.player_numbers[index],
                                                cx,
                                            )
                                            .w(px(100.)),
                                        )
                                    }),
                            ),
                    )
                    .into_any_element(),
            );
        }
        rows
    }

    pub fn reset_waveform(&mut self) {
        self.waveform_generation = self.waveform_generation.wrapping_add(1);
        self.waveform_dragging = false;
        self.waveform_preview = None;
        self.waveform_bounds.set(None);
        self.waveform_job = None;
        self.waveform_key.clear();
        self.waveform_data = None;
    }
    pub fn sync_waveform(&mut self, cx: &mut Context<Self>) {
        if self.cache_busy {
            return;
        }
        if self.state.settings.playerbar.kind != "waveform" || self.radio_item.is_some() {
            self.reset_waveform();
            return;
        }
        let Some(item) = self
            .current_entry()
            .filter(|entry| entry.item.kind == ItemKind::Song)
            .map(|entry| entry.item.clone())
        else {
            self.reset_waveform();
            return;
        };
        let key = format!(
            "stereo-v2-{}-{:x}-{}-{}",
            item.server_id,
            feishin_core::api::cover_hash(&item.id),
            item.duration.to_bits(),
            self.state.settings.transcode
        );
        let key = feishin_core::cache::versioned(&key, self.state.cache_revision);
        if key == self.waveform_key {
            return;
        }
        // A replacement track must stop displaying and decoding the old audio,
        // including while the replacement's loading delay is still running.
        if !self.waveform_key.is_empty() || self.waveform_data.is_some() {
            self.reset_waveform();
        }
        if self.player_title_at.elapsed().as_secs_f32() < self.state.settings.playerbar.load_delay()
        {
            return;
        }
        let Some(api) = self.sessions.get(&item.server_id).cloned() else {
            return;
        };
        self.reset_waveform();
        self.waveform_key = key.clone();
        let generation = self.waveform_generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.waveform_job = Some(Job(cancel.clone()));
        let mut settings = self.state.settings.clone();
        settings.bitrate = Some(64);
        settings.format = "mp3".into();
        let path = self.data_dir.join("waveforms").join(format!("{key}.json"));
        let stale_path = path.clone();
        let revision = self.state.cache_revision;
        let abort = cancel.clone();
        let task = cx.background_executor().spawn(async move {
            if let Ok(bytes) = std::fs::read(&path)
                && let Ok(data) = serde_json::from_slice::<Data>(&bytes)
                && data.validate()
            {
                return Ok(data);
            }
            let url = api.stream_url(&item, &settings)?;
            let data = feishin_core::playback::waveform(&url, cancel, settings.ignore_ssl)?;
            anyhow::ensure!(!abort.load(Ordering::Relaxed), "Waveform cancelled");
            feishin_core::storage::atomic_write(&path, &serde_json::to_vec(&data)?)?;
            Ok::<_, anyhow::Error>(data)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.state.cache_revision != revision {
                    feishin_core::cache::discard_stale(&stale_path);
                    return;
                }
                if this.waveform_key != key || this.waveform_generation != generation {
                    return;
                }
                this.waveform_job = None;
                match result {
                    Ok(data) => this.waveform_data = Some(data),
                    Err(error) => {
                        log::debug!("Waveform unavailable, using the seek slider: {error}")
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn waveform_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let data = self.waveform_data.clone().unwrap();
        let config = self.state.settings.playerbar.clone();
        let progress = (self
            .waveform_preview
            .unwrap_or_else(|| self.current_position())
            / data.duration)
            .clamp(0., 1.) as f32;
        let primary = cx.theme().primary;
        let cursor = if cx.theme().mode.is_dark() {
            white()
        } else {
            black()
        }
        .opacity(0.5);
        let zoom = self.state.settings.zoom;
        let layout = self.waveform_bounds.clone();
        let drag = cx.listener(|this, event: &MouseMoveEvent, _, cx| {
            if this.waveform_dragging && event.pressed_button == Some(MouseButton::Left) {
                this.preview_waveform(event.position, cx);
            }
        });
        div()
            .id("waveform-seek")
            .debug_selector(|| "waveform-seek".into())
            .w_full()
            .relative()
            .h(px(18. * zoom))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.preview_waveform(event.position, cx);
                    this.waveform_dragging = this.waveform_preview.is_some();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.finish_waveform_seek(window, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.finish_waveform_seek(window, cx)),
            )
            .child(
                canvas(
                    move |bounds, _, _| {
                        layout.set(Some(bounds));
                    },
                    move |bounds, _, window, _| {
                        window.on_mouse_event(move |event, phase, window, cx| {
                            if phase == DispatchPhase::Bubble {
                                drag(event, window, cx);
                            }
                        });
                        paint_waveform(
                            bounds,
                            &data,
                            &config,
                            progress,
                            [primary, cursor],
                            zoom,
                            window,
                        );
                    },
                )
                .size_full(),
            )
            .children(self.waveform_preview.map(|position| {
                div()
                    .id("waveform-seek-preview")
                    .debug_selector(|| "waveform-seek-preview".into())
                    .absolute()
                    .left(relative(progress))
                    .top(px(-40. * zoom))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(cx.theme().popover)
                    .text_color(cx.theme().popover_foreground)
                    .child(feishin_core::model::duration_label(position))
            }))
            .into_any_element()
    }
    fn preview_waveform(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(bounds) = self.waveform_bounds.get()
            && let Some(data) = self.waveform_data.as_ref().filter(|data| data.validate())
            && self
                .current_entry()
                .is_some_and(|entry| entry.item.kind == ItemKind::Song && entry.item.duration > 0.)
        {
            let fraction = (f32::from(position.x - bounds.origin.x)
                / f32::from(bounds.size.width).max(1.))
            .clamp(0., 1.);
            self.waveform_preview = Some(f64::from(fraction) * data.duration);
            cx.notify();
        }
    }
    fn finish_waveform_seek(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.waveform_dragging {
            self.waveform_dragging = false;
            if let Some(position) = self.waveform_preview.take()
                && let Some(duration) = self.waveform_data.as_ref().map(|data| data.duration)
            {
                self.seek_with_duration(position, duration, window, cx);
            }
            cx.notify();
        }
    }
    pub fn seek_position(&mut self, position: f64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(duration) = self
            .current_entry()
            .filter(|entry| entry.item.kind == ItemKind::Song)
            .map(|entry| entry.item.duration)
        else {
            return;
        };
        self.seek_with_duration(position, duration, window, cx);
    }
    fn seek_with_duration(
        &mut self,
        position: f64,
        duration: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = position.clamp(0., duration.max(0.));
        self.commit_seek(position, cx);
        self.sync_progress(window, cx);
    }
}
fn paint_waveform(
    bounds: Bounds<Pixels>,
    data: &Data,
    config: &Config,
    progress: f32,
    colors: [Hsla; 2],
    zoom: f32,
    window: &mut Window,
) {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let scale = (window.scale_factor() * zoom).max(1.);
    let canvases = data.drawing(
        config,
        f64::from(width / zoom),
        f64::from(height / zoom),
        f64::from(scale),
    );
    let draw = |window: &mut Window, color: Hsla| {
        for canvas in &canvases {
            let origin = bounds.origin + point(px(canvas.offset as f32 * zoom), px(0.));
            let mask = Bounds::new(
                origin,
                size(px(canvas.width as f32 * zoom), bounds.size.height),
            );
            window.with_content_mask(Some(ContentMask { bounds: mask }), |window| {
                for [x, y, width, height] in &canvas.bars {
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                origin
                                    + point(
                                        px(*x as f32 / scale * zoom),
                                        px(*y as f32 / scale * zoom),
                                    ),
                                size(
                                    px(*width as f32 / scale * zoom),
                                    px(*height as f32 / scale * zoom),
                                ),
                            ),
                            color,
                        )
                        .corner_radii(px(f64::from(config.bar_radius)
                            .min(width / 2.)
                            .min(height / 2.)
                            as f32
                            / scale
                            * zoom)),
                    );
                }
                for points in &canvas.paths {
                    let mut path = PathBuilder::fill();
                    for (index, [x, y]) in points.iter().enumerate() {
                        let point = origin
                            + point(px(*x as f32 / scale * zoom), px(*y as f32 / scale * zoom));
                        if index == 0 {
                            path.move_to(point);
                        } else {
                            path.line_to(point);
                        }
                    }
                    path.close();
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                }
            });
        }
    };
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        let remaining = Bounds::new(
            bounds.origin + point(px(width * progress), px(0.)),
            size(px(width * (1. - progress)), bounds.size.height),
        );
        window.with_content_mask(Some(ContentMask { bounds: remaining }), |window| {
            draw(window, rgb(0x606060).into())
        });
        let progress_bounds = Bounds::new(
            bounds.origin,
            size(px(width * progress), bounds.size.height),
        );
        window.with_content_mask(
            Some(ContentMask {
                bounds: progress_bounds,
            }),
            |window| draw(window, colors[0]),
        );
        window.paint_quad(fill(
            Bounds::new(
                bounds.origin + point(px((width - 2. * zoom).max(0.) * progress), px(0.)),
                size(px(2. * zoom), px(height)),
            ),
            colors[1],
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Route;
    use ::core::prelude::v1::test;
    use feishin_core::{
        model::Item,
        queue::Enqueue,
        storage::{self, State},
    };
    use gpui::accesskit::{Action, ActionData, ActionRequest, TreeId};
    use gpui_component::{Root, input::InputEvent};
    use std::{cell::RefCell, rc::Rc};

    fn node(
        cx: &mut VisualTestContext,
        role: Role,
        label: &str,
    ) -> Option<(gpui::accesskit::NodeId, gpui::accesskit::Node)> {
        cx.update(|window, _| {
            let tree = window.accessibility_tree_for_test().unwrap();
            let consumer = accesskit_consumer::Tree::new(tree.clone(), true);
            tree.nodes
                .iter()
                .find(|(id, node)| {
                    node.role() == role
                        && consumer
                            .state()
                            .node_by_tree_local_id(*id, TreeId::ROOT)
                            .unwrap()
                            .label()
                            .as_deref()
                            == Some(label)
                })
                .cloned()
        })
    }
    fn click(cx: &mut VisualTestContext, role: Role, label: &str) {
        let bounds = node(cx, role, label)
            .unwrap_or_else(|| panic!("Missing {role:?}: {label}"))
            .1
            .bounds()
            .unwrap();
        let point = cx.update(|window, _| {
            let scale = f64::from(window.scale_factor());
            point(
                px(((bounds.x0 + bounds.x1) / (2. * scale)) as f32),
                px(((bounds.y0 + bounds.y1) / (2. * scale)) as f32),
            )
        });
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
    }

    #[gpui::test]
    fn waveform_settings_match_original_controls_limits_callbacks_and_locales(
        cx: &mut TestAppContext,
    ) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/control-settings-reference.json"))
                .unwrap();
        let controls = reference
            .iter()
            .find(|case| case["type"] == "waveform")
            .unwrap()["controls"]
            .as_array()
            .unwrap();
        let dir = std::env::temp_dir().join(format!(
            "feishin-waveform-settings-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.queue.add(
                vec![
                    Item {
                        id: "duplicate".into(),
                        ..Default::default()
                    };
                    2
                ],
                Enqueue::End,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1500.), px(1600.)));
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        for language in ["en", "fr", "ar"] {
            let tr = |key: &str| feishin_core::i18n::text(language, key);
            for key in [
                "setting.playerbarSlider",
                "setting.playerbarWaveformAlign",
                "setting.playerbarWaveformBarWidth",
                "setting.playerbarWaveformGap",
                "setting.playerbarWaveformRadius",
                "setting.playerbarWaveformStretch",
                "setting.showSkipButtons",
                "setting.waveformLoadingDelay",
                "setting.skipDuration",
            ] {
                let label = tr(key);
                let control = controls
                    .iter()
                    .find(|control| control["title"]["key"] == key)
                    .unwrap();
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        view.route = Route::Settings;
                        view.state.settings.language = language.into();
                        view.state.settings.playerbar = Config {
                            kind: "waveform".into(),
                            ..Config::default()
                        };
                        view.settings_search
                            .update(cx, |input, cx| input.set_value(label.clone(), window, cx));
                        view.refresh_setting_inputs(window, cx);
                        view.apply_theme(window, cx);
                        view.save();
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                for (input_index, input) in control["inputs"].as_array().unwrap().iter().enumerate()
                {
                    match input["type"].as_str().unwrap() {
                        "segment" => {
                            for choice in input["choices"].as_array().unwrap() {
                                let choice_label = tr(&format!(
                                    "{}_{}",
                                    choice["label"]["key"].as_str().unwrap(),
                                    choice["label"]["options"]["context"].as_str().unwrap()
                                ));
                                click(cx, Role::Button, &choice_label);
                                assert_eq!(
                                    node(cx, Role::Button, &choice_label)
                                        .unwrap()
                                        .1
                                        .is_selected(),
                                    Some(true)
                                );
                                let saved = storage::load(&dir.join("state.json"))
                                    .unwrap()
                                    .settings
                                    .playerbar;
                                assert_eq!(
                                    if key == "setting.playerbarSlider" {
                                        saved.kind
                                    } else {
                                        saved.bar_align
                                    },
                                    choice["value"].as_str().unwrap()
                                );
                            }
                        }
                        "slider" => {
                            let (id, slider) = node(cx, Role::Slider, &label).unwrap();
                            assert_eq!(slider.min_numeric_value(), input["min"].as_f64());
                            assert_eq!(slider.max_numeric_value(), input["max"].as_f64());
                            for change in input["changes"].as_array().unwrap() {
                                cx.update(|window, cx| {
                                    window.dispatch_accessibility_action_for_test(
                                        ActionRequest {
                                            action: Action::SetValue,
                                            target_tree: TreeId::ROOT,
                                            target_node: id,
                                            data: Some(ActionData::NumericValue(
                                                change["value"].as_f64().unwrap(),
                                            )),
                                        },
                                        cx,
                                    )
                                });
                                cx.run_until_parked();
                                let saved = storage::load(&dir.join("state.json")).unwrap();
                                let field = match key {
                                    "setting.playerbarWaveformBarWidth" => "barWidth",
                                    "setting.playerbarWaveformGap" => "barGap",
                                    _ => "barRadius",
                                };
                                assert_eq!(
                                    serde_json::to_value(&saved.settings.playerbar).unwrap()[field]
                                        .as_f64(),
                                    change["settings"]["general"]["playerbarSlider"][field]
                                        .as_f64()
                                );
                            }
                            let index = match key {
                                "setting.playerbarWaveformBarWidth" => 0,
                                "setting.playerbarWaveformGap" => 1,
                                _ => 2,
                            };
                            let bounds = cx
                                .debug_bounds(&format!("waveform-setting-{index}"))
                                .unwrap();
                            let position =
                                point(bounds.left() + bounds.size.width * 0.25, bounds.center().y);
                            let before = app.read_with(cx, |view, _| {
                                serde_json::to_value(&view.state.settings.playerbar).unwrap()
                            });
                            cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
                            cx.run_until_parked();
                            assert_eq!(
                                app.read_with(cx, |view, _| serde_json::to_value(
                                    &view.state.settings.playerbar
                                )
                                .unwrap()),
                                before
                            );
                            cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
                            cx.run_until_parked();
                            assert_ne!(
                                app.read_with(cx, |view, _| serde_json::to_value(
                                    &view.state.settings.playerbar
                                )
                                .unwrap()),
                                before
                            );
                        }
                        "switch" => {
                            for change in input["changes"].as_array().unwrap() {
                                let checked = change["value"].as_bool().unwrap();
                                let current = node(cx, Role::CheckBox, &label).unwrap().1.toggled()
                                    == Some(gpui::accesskit::Toggled::True);
                                if current != checked {
                                    click(cx, Role::CheckBox, &label);
                                }
                                let config = storage::load(&dir.join("state.json"))
                                    .unwrap()
                                    .settings
                                    .playerbar;
                                assert_eq!(
                                    if key == "setting.playerbarWaveformStretch" {
                                        config.stretched
                                    } else {
                                        config.skip_enabled
                                    },
                                    checked
                                );
                            }
                        }
                        "number" => {
                            let index = if key == "setting.waveformLoadingDelay" {
                                4
                            } else {
                                5 + input_index
                            };
                            let number_label = if index == 4 {
                                label.clone()
                            } else {
                                tr(if index == 5 {
                                    "common.backward"
                                } else {
                                    "common.forward"
                                })
                            };
                            assert!(node(cx, Role::TextInput, &number_label).is_some());
                            for change in input["changes"].as_array().unwrap() {
                                let raw = change["value"].as_str().unwrap();
                                let before = app
                                    .read_with(cx, |view, _| view.state.settings.playerbar.clone());
                                cx.update(|window, cx| {
                                    app.read(cx).player_numbers[index].clone().update(
                                        cx,
                                        |input, cx| {
                                            input.set_value(raw.to_owned(), window, cx);
                                            cx.emit(InputEvent::Blur);
                                        },
                                    );
                                });
                                cx.run_until_parked();
                                let valid = raw.is_empty()
                                    || raw.parse::<f64>().is_ok_and(|value| {
                                        value.is_finite()
                                            && value >= 0.
                                            && (index != 4 || value <= 30.)
                                    });
                                let expected = if valid {
                                    let (group, field) = match index {
                                        4 => ("playerbarSlider", "loadingDelay"),
                                        5 => ("skipButtons", "skipBackwardSeconds"),
                                        _ => ("skipButtons", "skipForwardSeconds"),
                                    };
                                    change["settings"]["general"][group][field]
                                        .as_f64()
                                        .unwrap_or(input["value"].as_f64().unwrap())
                                } else {
                                    match index {
                                        4 => f64::from(before.loading_delay),
                                        5 => before.skip_backward_seconds,
                                        _ => before.skip_forward_seconds,
                                    }
                                };
                                let config = storage::load(&dir.join("state.json"))
                                    .unwrap()
                                    .settings
                                    .playerbar;
                                assert_eq!(
                                    match index {
                                        4 => f64::from(config.loading_delay),
                                        5 => config.skip_backward_seconds,
                                        _ => config.skip_forward_seconds,
                                    },
                                    expected
                                );
                            }
                        }
                        other => panic!("Unexpected waveform control {other}"),
                    }
                }
                let mut saved = storage::load(&dir.join("state.json")).unwrap();
                feishin_core::migration::validate(&mut saved.settings).unwrap();
                assert_eq!(serde_json::to_value(saved.queue).unwrap(), queue);
            }
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.playerbar.kind = "slider".into();
                    view.settings_search.update(cx, |input, cx| {
                        input.set_value(tr("setting.playerbarWaveformBarWidth"), window, cx)
                    });
                    cx.notify();
                })
            });
            cx.run_until_parked();
            assert!(node(cx, Role::Slider, &tr("setting.playerbarWaveformBarWidth")).is_none());
        }
        app.read_with(cx, |view, _| assert!(!view.playing));
        std::fs::remove_dir_all(dir).ok();
    }
}
