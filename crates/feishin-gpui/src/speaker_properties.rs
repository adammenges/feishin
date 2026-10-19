use crate::{app::Feishin, speaker_picker::same_id};
use feishin_core::dlna::{Controller, Device, Property};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Sizable, StyledExt,
    popover::Popover,
    slider::{SliderEvent, SliderState},
    switch::Switch,
};

pub(crate) struct PropertyWrite {
    device: Device,
    next: Vec<Property>,
}
fn kind(property: Property) -> u8 {
    match property {
        Property::Bass(_) => 0,
        Property::Treble(_) => 1,
        Property::Loudness(_) => 2,
        Property::Crossfade(_) => 3,
        Property::Led(_) => 4,
        Property::Touch(_) => 5,
    }
}
impl Feishin {
    pub(crate) fn speaker_property(
        &mut self,
        device: Device,
        property: Property,
        cx: &mut Context<Self>,
    ) {
        if !self.speaker_endpoint_current(&device) {
            return;
        }
        let id = device.id.clone();
        *self.cast_property_versions.entry(id.clone()).or_default() += 1;
        self.cast_property_loading.remove(&id);
        if let Some(properties) = self.cast_properties.get_mut(&id) {
            match property {
                Property::Bass(value) => properties.bass = Some(value),
                Property::Treble(value) => properties.treble = Some(value),
                Property::Loudness(value) => properties.loudness = Some(value),
                Property::Crossfade(value) => properties.crossfade = Some(value),
                Property::Led(value) => properties.led = Some(value),
                Property::Touch(value) => properties.touch = Some(value),
            }
        }
        if let Some(pending) = self
            .cast_property_pending
            .get_mut(&id)
            .filter(|pending| pending.device == device)
        {
            pending
                .next
                .retain(|queued| kind(*queued) != kind(property));
            pending.next.push(property);
        } else {
            self.cast_property_pending.insert(
                id,
                PropertyWrite {
                    device: device.clone(),
                    next: vec![],
                },
            );
            self.write_speaker_property(device, property, cx);
        }
        cx.notify();
    }
    fn write_speaker_property(&self, device: Device, property: Property, cx: &mut Context<Self>) {
        let target = device.clone();
        cx.spawn(async move |this, cx| {
            let result =
                smol::unblock(move || Controller::new()?.set_property(&target, property)).await;
            this.update(cx, |this, cx| {
                if !this.speaker_endpoint_current(&device) {
                    if this
                        .cast_property_pending
                        .get(&device.id)
                        .is_some_and(|pending| pending.device == device)
                    {
                        this.cast_property_pending.remove(&device.id);
                    }
                    return;
                }
                let Some(pending) = this
                    .cast_property_pending
                    .get_mut(&device.id)
                    .filter(|pending| pending.device == device)
                else {
                    return;
                };
                let next = if pending.next.is_empty() {
                    None
                } else {
                    Some(pending.next.remove(0))
                };
                if let Err(error) = result {
                    log::warn!("Speaker property update failed: {error}");
                    this.message = error.to_string();
                }
                if let Some(next) = next {
                    this.write_speaker_property(device, next, cx);
                } else {
                    this.cast_property_pending.remove(&device.id);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub(crate) fn speaker_endpoint_current(&self, device: &Device) -> bool {
        self.cast_devices
            .iter()
            .chain(self.cast_selected.iter())
            .chain(self.cast_members.values())
            .find(|candidate| same_id(&candidate.id, &device.id))
            .is_none_or(|candidate| {
                candidate.location == device.location
                    && candidate.control_url == device.control_url
                    && candidate.rendering_url == device.rendering_url
            })
    }
    pub(crate) fn speaker_properties_button(
        &self,
        device: Device,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app = cx.weak_entity();
        let open_device = device.clone();
        let id = device.id.clone();
        Popover::new(SharedString::from(format!("speaker-properties-{id}")))
            .anchor(if crate::appearance::rtl(cx) {
                Corner::TopLeft
            } else {
                Corner::TopRight
            })
            .centered(Axis::Vertical)
            // The original reserves half the seven-pixel arrow, then clips it with overflow-x.
            .offset(px(13.5 * self.state.settings.zoom))
            .viewport_margin(px(5. * self.state.settings.zoom))
            .transition_offset(point(px(30. * self.state.settings.zoom), px(0.)))
            .appearance(false)
            .trigger(
                crate::playerbar::button("speaker-properties", false, window, cx)
                    .debug_selector(move || format!("speaker-properties-{id}"))
                    .size(rems(1.5))
                    .with_size(window.rem_size() * 1.5)
                    .icon(
                        gpui_component::Icon::default()
                            .path("native-icons/settings.svg")
                            .with_size(window.rem_size()),
                    )
                    .tooltip(format!("{}: {}", self.tr("common.configure"), device.name)),
            )
            .on_open_change(cx.listener(move |app, open, _, cx| {
                if *open {
                    app.load_speaker_properties(open_device.clone(), cx);
                }
            }))
            .content(move |_, window, cx| {
                let app = app.clone();
                let device = device.clone();
                let initial = device.clone();
                let panel = window.use_keyed_state(
                    SharedString::from(format!("speaker-property-panel-{}", device.id)),
                    cx,
                    move |window, cx| PropertiesPanel::new(app, initial, window, cx),
                );
                panel.update(cx, |panel, _| panel.device = device);
                panel
            })
            .into_any_element()
    }
}
pub(crate) struct PropertiesPanel {
    app: WeakEntity<Feishin>,
    device: Device,
    sliders: [Entity<SliderState>; 2],
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}
impl PropertiesPanel {
    pub(crate) fn new(
        app: WeakEntity<Feishin>,
        device: Device,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sliders =
            std::array::from_fn(|_| cx.new(|_| SliderState::new().min(-10.).max(10.).step(1.)));
        let mut subscriptions = Vec::new();
        for (index, slider) in sliders.iter().enumerate() {
            subscriptions.push(
                cx.subscribe_in(slider, window, move |panel, _, event, _, cx| {
                    if let SliderEvent::Change(value) = event {
                        let value = value.start().round() as i32;
                        panel
                            .app
                            .update(cx, |app, cx| {
                                app.speaker_property(
                                    panel.device.clone(),
                                    if index == 0 {
                                        Property::Bass(value)
                                    } else {
                                        Property::Treble(value)
                                    },
                                    cx,
                                )
                            })
                            .ok();
                    }
                }),
            );
        }
        Self {
            app,
            device,
            sliders,
            scroll: ScrollHandle::default(),
            _subscriptions: subscriptions,
        }
    }
}
impl Render for PropertiesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = self.app.upgrade() else {
            return div().into_any_element();
        };
        let (properties, loading, zoom) = app.read_with(cx, |app, _| {
            (
                app.cast_properties.get(&self.device.id).cloned(),
                app.cast_property_loading.contains(&self.device.id),
                app.state.settings.zoom,
            )
        });
        let label = |key| crate::appearance::text(key, cx);
        let rtl = crate::appearance::rtl(cx);
        let mut panel = div()
            .id("speaker-property-panel")
            .debug_selector(|| "speaker-property-panel".into())
            .role(Role::Dialog)
            .aria_label(self.device.name.clone())
            .v_flex()
            .relative()
            .gap(rems(0.5))
            .w(rems(21.25))
            .max_h(window.viewport_size().height * 0.8)
            .overflow_x_hidden()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .map(|panel| {
                crate::speaker_picker::popup_surface(
                    panel,
                    "speaker-property-panel",
                    window,
                    zoom,
                    cx,
                )
            })
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex_shrink_0()
                    .debug_selector({
                        let name = self.device.name.clone();
                        move || format!("property-text-{name}")
                    })
                    .text_size(rems(13. / 16.))
                    .line_height(rems(1.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_center()
                    .child(self.device.name.clone()),
            );
        let surface = cx.theme().secondary;
        let paper = |index| {
            div()
                .v_flex()
                .flex_shrink_0()
                .debug_selector(move || format!("property-paper-{index}"))
                .p(rems(0.75))
                .rounded(rems(0.375))
                .bg(surface)
        };
        if loading {
            panel = panel.child(paper(0).child(property_status(
                label("dlna.speakerProperties.loading"),
                cx.theme().muted_foreground,
            )));
        } else if let Some(properties) = properties.filter(|p| {
            p.bass.is_some()
                || p.treble.is_some()
                || p.loudness.is_some()
                || p.crossfade.is_some()
                || p.led.is_some()
                || p.touch.is_some()
        }) {
            let mut sliders = paper(0);
            for (index, (value, name)) in [(properties.bass, "bass"), (properties.treble, "treble")]
                .into_iter()
                .enumerate()
            {
                if let Some(value) = value {
                    self.sliders[index].update(cx, |slider, cx| {
                        if slider.value().start() != value as f32 {
                            slider.set_value(value as f32, window, cx);
                        }
                    });
                    let prefix = if index == 0 {
                        "speaker-property-bass"
                    } else {
                        "speaker-property-treble"
                    };
                    sliders = sliders.child(property_row(
                        name,
                        crate::appearance::text(&format!("dlna.speakerProperties.{name}"), cx),
                        div()
                            .w(rems(10.))
                            .child(
                                crate::styles::slider(&self.sliders[index], cx)
                                    .settings(zoom, prefix)
                                    .aria_label(crate::appearance::text(
                                        &format!("dlna.speakerProperties.{name}"),
                                        cx,
                                    ))
                                    .value_label(value.to_string()),
                            )
                            .into_any_element(),
                        true,
                        rtl,
                    ));
                }
            }
            let mut toggles = paper(1);
            for (value, name, property) in [
                (
                    properties.loudness,
                    "loudness",
                    Property::Loudness as fn(bool) -> Property,
                ),
                (properties.crossfade, "crossfade", Property::Crossfade),
                (properties.led, "ledState", Property::Led),
                (properties.touch, "touchControls", Property::Touch),
            ] {
                if let Some(value) = value {
                    let target = self.device.clone();
                    let app = app.clone();
                    let prefix = match name {
                        "loudness" => "speaker-property-loudness",
                        "crossfade" => "speaker-property-crossfade",
                        "ledState" => "speaker-property-ledState",
                        _ => "speaker-property-touchControls",
                    };
                    let label =
                        crate::appearance::text(&format!("dlna.speakerProperties.{name}"), cx);
                    toggles = toggles.child(property_row(
                        name,
                        label.clone(),
                        Switch::new(name)
                            .settings(zoom, rtl, prefix)
                            .tooltip(label)
                            .checked(value)
                            .on_click(move |value, _, cx| {
                                app.update(cx, |app, cx| {
                                    app.speaker_property(target.clone(), property(*value), cx)
                                })
                            })
                            .into_any_element(),
                        false,
                        rtl,
                    ));
                }
            }
            panel = panel.child(sliders).child(toggles);
        } else {
            panel = panel.child(paper(0).child(property_status(
                label("dlna.speakerProperties.loadFailed"),
                cx.theme().danger,
            )));
        }
        panel
            .child(crate::scroll::Scrollbar::vertical(
                "speaker-properties-scrollbar",
                &self.scroll,
            ))
            .into_any_element()
    }
}

fn property_status(label: String, color: Hsla) -> impl IntoElement {
    div()
        .debug_selector({
            let label = label.clone();
            move || format!("property-text-{label}")
        })
        .text_size(rems(11. / 16.))
        .line_height(rems(14. / 16.))
        .text_center()
        .text_color(color)
        .child(label)
}
fn property_row(
    name: &'static str,
    label: String,
    control: AnyElement,
    slider: bool,
    rtl: bool,
) -> impl IntoElement {
    div()
        .debug_selector(move || format!("property-row-{name}"))
        .h_flex()
        .map(|mut row| {
            row.style().align_items = Some(AlignItems::Stretch);
            row
        })
        .when(rtl, |this| this.flex_row_reverse())
        .text_size(rems(0.875))
        .line_height(rems(1.125))
        .child(
            div()
                .debug_selector({
                    let label = label.clone();
                    move || format!("property-text-{label}")
                })
                .flex_1()
                .min_w_0()
                .v_flex()
                .justify_center()
                .py(rems(0.75))
                .pr(rems(0.75))
                .font_weight(FontWeight::MEDIUM)
                .when(rtl, |this| this.text_right())
                .child(label),
        )
        .child(
            div()
                .h_flex()
                .items_start()
                .py(rems(0.75))
                .flex_shrink_0()
                .when(!slider, |this| this.w(relative(0.5)))
                .justify_end()
                .when(rtl, |this| this.justify_start())
                .child(control),
        )
}
