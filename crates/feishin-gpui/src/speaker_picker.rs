use crate::app::Feishin;
use feishin_core::dlna::{Controller, Device, Group, Member};
use gpui::Context;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) fn same_id(a: &str, b: &str) -> bool {
    a.trim_start_matches("uuid:") == b.trim_start_matches("uuid:")
}

#[derive(Clone, Default)]
pub(crate) struct GroupSelection {
    pub selected: Vec<Device>,
    pub locked: Option<Device>,
}
impl GroupSelection {
    pub fn new(locked: Option<Device>) -> Self {
        Self {
            selected: locked.iter().cloned().collect(),
            locked,
        }
    }
    pub fn coordinator(&self) -> Option<&Device> {
        self.locked.as_ref().or_else(|| self.selected.first())
    }
    pub fn can_confirm(&self) -> bool {
        self.selected.len() >= 2
    }
    pub fn toggle(&mut self, device: Device, pair: bool) {
        if pair
            || self
                .locked
                .as_ref()
                .is_some_and(|locked| same_id(&locked.id, &device.id))
        {
            return;
        }
        if let Some(index) = self
            .selected
            .iter()
            .position(|item| same_id(&item.id, &device.id))
        {
            self.selected.remove(index);
        } else {
            self.selected.push(device);
        }
    }
}

impl Feishin {
    pub(crate) fn cancel_speaker_group(&mut self) {
        if let Some(cancel) = self.cast_group_pending.take() {
            cancel.store(true, Ordering::SeqCst);
        }
    }
    pub(crate) fn confirm_speaker_group(&mut self, cx: &mut Context<Self>) {
        if self.cast_group_pending.is_some() || self.cast_connecting.is_some() {
            return;
        }
        let Some(builder) = self
            .cast_builder
            .clone()
            .filter(GroupSelection::can_confirm)
        else {
            return;
        };
        let Some(coordinator) = builder.coordinator().cloned() else {
            return;
        };
        if builder.locked.is_some() {
            self.add_speaker_group(coordinator, builder.selected, cx);
        } else {
            self.connect_speaker_with_group(coordinator, Some(builder.selected), cx);
        }
    }
    pub(crate) fn add_speaker_group(
        &mut self,
        coordinator: Device,
        selected: Vec<Device>,
        cx: &mut Context<Self>,
    ) {
        self.cancel_speaker_group();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cast_group_pending = Some(cancel.clone());
        let target = coordinator.clone();
        cx.spawn(async move |this, cx| {
            let stop = cancel.clone();
            let result = smol::unblock(move || {
                let controller = Controller::new()?;
                let mut joined = Vec::new();
                let mut errors = Vec::new();
                for device in selected {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if same_id(&device.id, &target.id) {
                        continue;
                    }
                    match controller.join(&device, &target) {
                        Ok(()) => joined.push(device),
                        Err(error) => {
                            log::warn!("Failed to add speaker {}: {error}", device.name);
                            errors.push(device.name);
                        }
                    }
                }
                Ok::<_, anyhow::Error>((joined, errors))
            })
            .await;
            this.update(cx, |this, cx| {
                if this
                    .cast_group_pending
                    .as_ref()
                    .is_none_or(|pending| !Arc::ptr_eq(pending, &cancel))
                {
                    return;
                }
                this.cast_group_pending = None;
                if this.cast_selected.as_ref() != Some(&coordinator) {
                    return;
                }
                this.cast_builder = None;
                match result {
                    Ok((joined, errors)) => {
                        this.cast_topology_generation += 1;
                        let index = this
                            .cast_groups
                            .iter()
                            .position(|group| same_id(&group.coordinator, &coordinator.id));
                        let index = index.unwrap_or_else(|| {
                            this.cast_groups.push(Group {
                                id: coordinator.id.clone(),
                                coordinator: coordinator.id.clone(),
                                stereo_pair: false,
                                members: vec![Member {
                                    id: coordinator.id.clone(),
                                    name: coordinator.name.clone(),
                                    location: coordinator.location.clone(),
                                    satellite: false,
                                }],
                            });
                            this.cast_groups.len() - 1
                        });
                        this.cast_members
                            .insert(coordinator.id.clone(), coordinator.clone());
                        for device in joined {
                            if !this.cast_groups[index]
                                .members
                                .iter()
                                .any(|member| same_id(&member.id, &device.id))
                            {
                                this.cast_groups[index].members.push(Member {
                                    id: device.id.clone(),
                                    name: device.name.clone(),
                                    location: device.location.clone(),
                                    satellite: false,
                                });
                                this.cast_member_volumes.insert(device.id.clone(), 50);
                            }
                            this.cast_members.insert(device.id.clone(), device);
                        }
                        if !errors.is_empty() {
                            this.message = errors
                                .into_iter()
                                .map(|name| {
                                    this.tr("dlna.group.failedToAddMessage")
                                        .replace("{{name}}", &name)
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                        }
                    }
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_speaker::Speaker;
    use ::core::prelude::v1::test;
    use serde_json::{Value, json};

    #[test]
    fn network_group_builder_matches_original_selection_and_confirmation() {
        let speaker = Speaker::new();
        let reference: Value = serde_json::from_str(include_str!(
            "../assets/network-group-builder-reference.json"
        ))
        .unwrap();
        for case in reference.as_array().unwrap() {
            let locked = case["locked"].as_bool().unwrap();
            let pair = case["pair"].as_bool().unwrap();
            let mut builder = GroupSelection::new(locked.then(|| speaker.device("a")));
            for id in case["toggles"].as_array().unwrap() {
                let id = id.as_str().unwrap();
                builder.toggle(speaker.device(id), pair && id == "c");
            }
            let coordinator = builder
                .coordinator()
                .map(|device| device.id.trim_start_matches("uuid:"));
            let rows: Vec<_> = ["a", "b", "c"].into_iter().map(|id| json!({"id":id,"checked":builder.selected.iter().any(|d|same_id(&d.id,id)),"disabled":(locked && id=="a") || (pair && id=="c"),"coordinator":coordinator==Some(id)})).collect();
            assert_eq!(json!(rows), case["rows"], "{case}");
            assert_eq!(
                builder.can_confirm(),
                case["canConfirm"].as_bool().unwrap(),
                "{case}"
            );
            let confirmed = if builder.can_confirm() {
                json!({"selected":builder.selected.iter().map(|d|d.id.trim_start_matches("uuid:")).collect::<Vec<_>>(),"coordinator":coordinator})
            } else {
                Value::Null
            };
            assert_eq!(confirmed, case["confirmed"], "{case}");
        }
    }
}

use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    animation::CssTransition,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    checkbox::Checkbox,
};

pub(crate) fn popup_border(window: &Window, zoom: f32) -> Pixels {
    let scale = window.scale_factor();
    px((2. * zoom * scale).floor() / scale)
}

pub(crate) fn popup_surface<T: Styled>(
    element: T,
    id: &str,
    window: &Window,
    zoom: f32,
    cx: &App,
) -> T {
    let element = element
        .p(rems(0.5))
        .border(popup_border(window, zoom))
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .rounded(rems(0.375))
        .shadow(vec![BoxShadow {
            color: black().opacity(0.4),
            offset: point(px(2. * zoom), px(2. * zoom)),
            blur_radius: px(10. * zoom),
            spread_radius: px(2. * zoom),
        }])
        .foreground_filters(vec![BackdropFilterOperation::DropShadow {
            offset: [0., 0.],
            sigma: 5. * zoom,
            color: Some([0., 0., 0., 0.5]),
        }]);
    crate::styles::apply(
        element,
        "div",
        id,
        &["fs-popover-module-dropdown", "mantine-Popover-dropdown"],
        None,
        cx,
    )
}
pub(crate) fn spinner(size: Pixels) -> impl IntoElement {
    glyph("spinner", size).with_animation(
        "speaker-spinner",
        Animation::new(std::time::Duration::from_secs(1)).repeat(),
        |icon, progress| icon.rotate(Radians(progress * std::f32::consts::TAU)),
    )
}

fn row(cx: &App) -> Div {
    div()
        .h_flex()
        .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
}
fn text(label: String, small: bool) -> Div {
    let selector = format!("picker-text-{label}");
    div()
        .debug_selector(move || selector.clone())
        .text_size(rems(if small { 11. / 16. } else { 13. / 16. }))
        .line_height(rems(if small { 14. / 16. } else { 1. }))
        .whitespace_nowrap()
        .child(label)
}
fn heading(label: String) -> Div {
    text(label, false)
        .font_weight(FontWeight::SEMIBOLD)
        .text_center()
        .pb(rems(0.75))
}
fn button(
    id: impl Into<ElementId>,
    label: String,
    outlined: bool,
    window: &Window,
    cx: &App,
    zoom: f32,
) -> Button {
    let id = id.into();
    let selector = id.to_string();
    Button::new(id)
        .debug_selector(move || selector.clone())
        .label(label)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(transparent_black())
                .foreground(cx.theme().foreground)
                .hover(cx.theme().secondary)
                .active(cx.theme().secondary)
                .border(if outlined {
                    cx.theme().border
                } else {
                    transparent_black()
                }),
        )
        .border(px(if window.scale_factor() >= 2. {
            zoom
        } else {
            zoom.floor().max(1.)
        }))
        .with_size(window.rem_size() * 1.875)
        .h(rems(1.875))
        .min_w_0()
        .text_size(rems(11. / 16.))
        .font_weight(FontWeight::MEDIUM)
        .line_height(relative(1.))
        .px(rems(0.875))
        .py_0()
        .rounded(window.rem_size() * (3. / 16.))
}
fn with_glyph(button: Button, name: &'static str, zoom: f32, cx: &App) -> Button {
    button
        .when(!crate::appearance::rtl(cx), |button| {
            button.icon(glyph(name, px(12. * zoom)))
        })
        .when(crate::appearance::rtl(cx), |button| {
            button.child(glyph(name, px(12. * zoom))).pr(rems(7. / 12.))
        })
        .when(!crate::appearance::rtl(cx), |button| {
            button.pl(rems(7. / 12.))
        })
}
fn glyph(name: &'static str, size: Pixels) -> Icon {
    Icon::default()
        .path(format!("native-icons/{name}.svg"))
        .with_size(size)
}

impl Feishin {
    fn picker_devices(&self) -> Vec<(Device, Option<Group>)> {
        let mut devices = self
            .cast_devices
            .iter()
            .filter(|device| {
                !self.cast_discovery_groups.iter().any(|group| {
                    group.members.len() > 1
                        && group
                            .members
                            .iter()
                            .any(|member| same_id(&member.id, &device.id))
                })
            })
            .cloned()
            .map(|device| (device, None))
            .collect::<Vec<_>>();
        for group in &self.cast_discovery_groups {
            if group.members.len() < 2 {
                continue;
            }
            if let Some(device) = self
                .cast_devices
                .iter()
                .find(|device| same_id(&device.id, &group.coordinator))
            {
                let mut device = device.clone();
                device.name = if group.stereo_pair {
                    format!(
                        "{} (Stereo Pair)",
                        group
                            .members
                            .first()
                            .map_or(device.name.as_str(), |member| &member.name)
                    )
                } else {
                    format!(
                        "Group ({})",
                        group.members.iter().filter(|m| !m.satellite).count()
                    )
                };
                devices.push((device, Some(group.clone())));
            }
        }
        devices
    }
    pub(crate) fn speaker_picker_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let zoom = self.state.settings.zoom;
        let hint_visible = f32::from(!window.modifiers().shift);
        let hint = window.use_keyed_state("speaker-disconnect-hint", cx, |_, _| {
            CssTransition::new(
                hint_visible,
                std::time::Duration::from_millis(150),
                [0.25, 0.1, 0.25, 1.],
            )
        });
        let hint_opacity = hint.update(cx, |transition, cx| {
            let now = cx.background_executor().now();
            transition.retarget(f32::from(!window.modifiers().shift), now);
            let (value, animating) = transition.value(now);
            if animating {
                window.request_animation_frame();
            }
            value
        });
        let busy = self.cast_connecting.is_some() || self.cast_group_pending.is_some();
        let devices = self.picker_devices();
        let mut paper = div()
            .id("speaker-picker-paper")
            .debug_selector(|| "speaker-picker-paper".into())
            .v_flex()
            .p(rems(0.75))
            .rounded(rems(0.375))
            .bg(cx.theme().secondary);
        let refresh = || {
            button(
                "picker-refresh",
                self.tr("dlna.group.refresh"),
                true,
                window,
                cx,
                zoom,
            )
            .flex_1()
            .map(|button| with_glyph(button, "refresh", self.state.settings.zoom, cx))
            .disabled(self.cast_scanning)
            .on_click(cx.listener(|app, _, _, cx| app.discover_speakers(cx)))
        };
        if busy {
            paper = paper.child(
                row(cx)
                    .p(rems(0.5))
                    .gap(rems(0.75))
                    .child(spinner(window.rem_size() * 0.75))
                    .child(
                        text(self.tr("dlna.connecting"), false)
                            .text_size(rems(0.875))
                            .line_height(rems(1.125))
                            .text_color(cx.theme().muted_foreground),
                    ),
            );
        } else if let Some(builder) = &self.cast_builder {
            paper = paper.child(heading(self.tr(if builder.locked.is_some() {
                "dlna.group.addSpeakers"
            } else {
                "dlna.group.selectSpeakers"
            })));
            if self.cast_scanning {
                paper = paper.child(self.picker_searching(true, window, cx));
            }
            let group = self.speaker_volume_group();
            let choices = devices
                .iter()
                .filter(|(device, topology)| {
                    builder.locked.as_ref().is_none_or(|coordinator| {
                        same_id(&coordinator.id, &device.id)
                            || (device.sonos
                                && topology.as_ref().is_none_or(|group| group.stereo_pair)
                                && group.is_none_or(|group| {
                                    !group
                                        .members
                                        .iter()
                                        .any(|member| same_id(&member.id, &device.id))
                                }))
                    })
                })
                .collect::<Vec<_>>();
            if !self.cast_scanning && choices.is_empty() {
                paper = paper.child(self.picker_empty(true, window, cx));
            }
            for (device, topology) in choices {
                let pair = topology.as_ref().is_some_and(|group| group.stereo_pair);
                let checked = builder
                    .selected
                    .iter()
                    .any(|selected| same_id(&selected.id, &device.id));
                let locked = builder
                    .locked
                    .as_ref()
                    .is_some_and(|selected| same_id(&selected.id, &device.id));
                let coordinator = builder
                    .coordinator()
                    .is_some_and(|selected| same_id(&selected.id, &device.id));
                let target = device.clone();
                paper = paper.child(
                    Checkbox::new(SharedString::from(format!("picker-check-{}", device.id)))
                        .debug_selector({
                            let id = device.id.clone();
                            move || format!("picker-check-{id}")
                        })
                        .aria_label(device.name.clone())
                        .checked(checked)
                        .disabled(locked || pair)
                        .with_size(window.rem_size() * 1.25)
                        .items_center()
                        .when(crate::appearance::rtl(cx), |checkbox| {
                            checkbox.flex_row_reverse()
                        })
                        .gap(rems(0.5))
                        .px(rems(0.5))
                        .py(rems(0.25))
                        .rounded(rems(3. / 16.))
                        .when(checked, |checkbox| {
                            checkbox.bg(cx.theme().primary.opacity(0.15))
                        })
                        .child(
                            row(cx)
                                .gap(rems(0.25))
                                .child(text(device.name.clone(), false))
                                .when(coordinator, |row| {
                                    row.child(
                                        text(self.tr("dlna.group.coordinator"), true)
                                            .text_color(cx.theme().primary),
                                    )
                                }),
                        )
                        .on_click(cx.listener(move |app, _, _, cx| {
                            if let Some(builder) = app.cast_builder.as_mut() {
                                builder.toggle(target.clone(), pair);
                            }
                            cx.notify();
                        })),
                );
            }
            if builder.locked.is_none() {
                paper = paper.child(
                    text(self.tr("dlna.group.firstSelectedIsCoordinator"), true)
                        .px(rems(0.5))
                        .text_color(cx.theme().muted_foreground),
                );
            }
            paper = paper.child(
                row(cx)
                    .gap(rems(0.25))
                    .mt(rems(0.5))
                    .child(refresh())
                    .child(
                        button(
                            "picker-cancel",
                            self.tr("dlna.group.cancel"),
                            true,
                            window,
                            cx,
                            zoom,
                        )
                        .flex_1()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.cast_builder = None;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "picker-confirm",
                            self.tr(if builder.locked.is_some() {
                                "dlna.group.add"
                            } else {
                                "dlna.group.connect"
                            }),
                            false,
                            window,
                            cx,
                            zoom,
                        )
                        .primary()
                        .flex_1()
                        .disabled(!builder.can_confirm())
                        .on_click(cx.listener(|app, _, _, cx| app.confirm_speaker_group(cx))),
                    ),
            );
        } else if let Some(selected) = &self.cast_selected {
            let group = self.speaker_volume_group();
            let members = group.map_or_else(
                || vec![selected.clone()],
                |group| {
                    group
                        .members
                        .iter()
                        .filter(|member| !member.satellite)
                        .filter_map(|member| {
                            self.cast_members
                                .values()
                                .chain(std::iter::once(selected))
                                .find(|device| same_id(&member.id, &device.id))
                                .cloned()
                        })
                        .collect::<Vec<_>>()
                },
            );
            let grouped = members.len() > 1;
            paper = paper.child(heading(if grouped {
                self.tr("dlna.group.title")
                    .replace("{{count}}", &members.len().to_string())
            } else {
                self.tr("dlna.nowCasting")
            }));
            for member in members {
                let coordinator = same_id(&member.id, &selected.id);
                let target = member.clone();
                let mut actions = row(cx).gap(rems(0.25));
                let pair = self
                    .cast_discovery_groups
                    .iter()
                    .any(|group| group.stereo_pair && same_id(&group.coordinator, &member.id));
                if grouped && !coordinator && !pair {
                    actions = actions.child(
                        button(
                            SharedString::from(format!("picker-remove-{}", member.id)),
                            self.tr("dlna.group.remove"),
                            false,
                            window,
                            cx,
                            zoom,
                        )
                        .h(rems(1.375))
                        .px(rems(0.4375))
                        .text_color(cx.theme().danger)
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.speaker_group(target.clone(), true, cx)
                        })),
                    );
                }
                if member.sonos {
                    actions =
                        actions.child(self.speaker_properties_button(member.clone(), window, cx));
                }
                paper = paper.child(
                    row(cx)
                        .justify_between()
                        .when(grouped, |row| row.px(rems(0.5)).py(rems(0.25)))
                        .child(
                            row(cx)
                                .gap(rems(0.75))
                                .child(text(member.name.clone(), false).text_color(
                                    if grouped && coordinator {
                                        cx.theme().primary
                                    } else if grouped {
                                        cx.theme().foreground
                                    } else {
                                        cx.theme().muted_foreground
                                    },
                                ))
                                .when(grouped && coordinator, |row| {
                                    row.child(glyph("star", px(12. * self.state.settings.zoom)))
                                }),
                        )
                        .child(actions),
                );
            }
            paper = paper
                .child(
                    row(cx)
                        .gap(rems(0.25))
                        .mt(rems(0.5))
                        .when(selected.sonos, |row| {
                            row.child(
                                button(
                                    "picker-expand",
                                    self.tr(if grouped {
                                        "dlna.group.addSpeaker"
                                    } else {
                                        "dlna.group.addToGroup"
                                    }),
                                    true,
                                    window,
                                    cx,
                                    zoom,
                                )
                                .flex_1()
                                .when(!grouped, |button| {
                                    with_glyph(button, "group", self.state.settings.zoom, cx)
                                })
                                .on_click(cx.listener(
                                    |app, _, _, cx| {
                                        app.cast_builder =
                                            Some(GroupSelection::new(app.cast_selected.clone()));
                                        app.discover_speakers(cx);
                                        cx.notify();
                                    },
                                )),
                            )
                        })
                        .child(
                            button(
                                "speaker-disconnect",
                                self.tr("dlna.disconnect"),
                                true,
                                window,
                                cx,
                                zoom,
                            )
                            .flex_1()
                            .when(!window.modifiers().shift, |button| {
                                button.text_color(cx.theme().danger)
                            })
                            .on_click(cx.listener(
                                |app, event: &ClickEvent, _, cx| {
                                    app.disconnect_speaker(event.modifiers().shift, cx)
                                },
                            )),
                        ),
                )
                .child(
                    text(self.tr("dlna.shiftDisconnectHint"), true)
                        .text_center()
                        .mt(rems(0.375))
                        .text_color(cx.theme().muted_foreground)
                        .opacity(hint_opacity),
                );
        } else {
            paper = paper.child(heading(self.tr("dlna.devices")));
            for (device, group) in devices
                .iter()
                .filter(|(_, group)| group.as_ref().is_some_and(|group| !group.stereo_pair))
            {
                let group = group.as_ref().unwrap();
                let target = device.clone();
                let mut card = div()
                    .id(SharedString::from(format!("picker-group-{}", device.id)))
                    .cursor_pointer()
                    .rounded(px(6. * zoom))
                    .mb(px(4. * zoom))
                    .py(px(8. * zoom))
                    .px(px(10. * zoom))
                    .child(text(device.name.clone(), false).font_weight(FontWeight::SEMIBOLD))
                    .on_click(
                        cx.listener(move |app, _, _, cx| app.connect_speaker(target.clone(), cx)),
                    );
                for member in group.members.iter().filter(|m| !m.satellite) {
                    card = card.child(
                        text(member.name.clone(), true)
                            .pl(px(8. * zoom))
                            .font_weight(if same_id(&member.id, &group.coordinator) {
                                FontWeight::BOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(cx.theme().muted_foreground),
                    );
                }
                paper = paper.child(card);
            }
            if self.cast_scanning {
                paper = paper.child(self.picker_searching(false, window, cx));
            } else if devices.is_empty() {
                paper = paper.child(self.picker_empty(false, window, cx));
            } else {
                for (device, _) in devices
                    .iter()
                    .filter(|(_, group)| group.as_ref().is_none_or(|group| group.stereo_pair))
                {
                    let target = device.clone();
                    paper =
                        paper.child(
                            button(
                                SharedString::from(format!("picker-connect-{}", device.id)),
                                device.name.clone(),
                                false,
                                window,
                                cx,
                                zoom,
                            )
                            .w_full()
                            .h(rems(2.25))
                            .justify_start()
                            .text_size(rems(0.8125))
                            .px(rems(1.125))
                            .on_click(cx.listener(
                                move |app, _, _, cx| app.connect_speaker(target.clone(), cx),
                            )),
                        );
                }
            }
            if !self.cast_scanning {
                paper = paper.child(row(cx).gap(rems(0.25)).mt(rems(0.5)).child(refresh()).when(
                    devices.len() >= 2 && devices.iter().any(|(device, _)| device.sonos),
                    |row| {
                        row.child(
                            button(
                                "picker-create",
                                self.tr("dlna.createGroup"),
                                true,
                                window,
                                cx,
                                zoom,
                            )
                            .flex_1()
                            .map(|button| with_glyph(button, "group", self.state.settings.zoom, cx))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.cast_builder = Some(GroupSelection::default());
                                cx.notify();
                            })),
                        )
                    },
                ))
            }
        }
        div()
            .id("player-output-panel")
            .debug_selector(|| "player-output-panel".into())
            .role(Role::Dialog)
            .aria_label(self.tr("dlna.castToDevice"))
            .min_w(px(340. * zoom))
            .max_w(window.viewport_size().width - px(16.))
            .max_h(window.viewport_size().height * 0.8)
            .on_modifiers_changed(cx.listener(|_, _, _, cx| cx.notify()))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(paper)
            .map(|panel| popup_surface(panel, "player-output-panel", window, zoom, cx))
            .into_any_element()
    }
    fn picker_searching(&self, group: bool, window: &Window, cx: &App) -> Div {
        row(cx)
            .p(rems(0.5))
            .gap(rems(0.75))
            .when(!group, |row| row.justify_center())
            .child(spinner(window.rem_size() * 0.75))
            .child(
                text(
                    self.tr(if group {
                        "dlna.group.searching"
                    } else {
                        "dlna.searching"
                    }),
                    false,
                )
                .text_size(rems(0.875))
                .line_height(rems(1.125))
                .text_color(cx.theme().muted_foreground),
            )
    }
    fn picker_empty(&self, group: bool, _window: &Window, cx: &App) -> Div {
        row(cx)
            .p(rems(0.5))
            .gap(rems(0.75))
            .child(glyph("circle-slash", px(12. * self.state.settings.zoom)))
            .child(
                text(
                    self.tr(if group {
                        "dlna.group.noSonosFound"
                    } else {
                        "dlna.noDevicesFound"
                    }),
                    false,
                )
                .text_size(rems(0.875))
                .line_height(rems(1.125))
                .text_color(cx.theme().muted_foreground),
            )
    }
}

#[cfg(all(test, unix))]
#[path = "speaker_picker_tests.rs"]
mod picker_tests;

impl Feishin {
    pub(crate) fn load_discovery_topology(&self, cx: &mut Context<Self>) {
        let known = self.cast_devices.clone();
        if !known.iter().any(|device| device.sonos) {
            return;
        }
        let generation = self.cast_discovery_generation;
        cx.spawn(async move |this, cx| {
            for delay in [1, 2, 4, 6] {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(delay))
                    .await;
                if !this
                    .update(cx, |this, _| this.cast_discovery_generation == generation)
                    .unwrap_or(false)
                {
                    return;
                }
                let known = known.clone();
                let result = smol::unblock(move || {
                    let controller = Controller::new()?;
                    let mut best = vec![];
                    for device in known.iter().filter(|device| device.sonos) {
                        if let Ok(groups) = controller.groups(device)
                            && groups.iter().map(|g| g.members.len()).sum::<usize>()
                                > best.iter().map(|g: &Group| g.members.len()).sum::<usize>()
                        {
                            best = groups;
                        }
                    }
                    let mut devices = known;
                    for member in best
                        .iter()
                        .flat_map(|group| &group.members)
                        .filter(|m| !m.satellite)
                    {
                        if devices.iter().any(|device| same_id(&device.id, &member.id)) {
                            continue;
                        }
                        match controller.describe(&member.location) {
                            Ok(device) => devices.push(device),
                            Err(error) => {
                                log::debug!("Discovered group member unavailable: {error}")
                            }
                        }
                    }
                    Ok::<_, anyhow::Error>((best, devices))
                })
                .await;
                let done = this
                    .update(cx, |this, cx| {
                        if this.cast_discovery_generation != generation {
                            return true;
                        }
                        match result {
                            Ok((groups, devices)) if !groups.is_empty() => {
                                this.cast_discovery_groups = groups;
                                this.cast_devices = devices;
                                cx.notify();
                                true
                            }
                            Err(error) => {
                                log::debug!("Speaker discovery topology unavailable: {error}");
                                false
                            }
                            _ => false,
                        }
                    })
                    .unwrap_or(true);
                if done {
                    return;
                }
            }
        })
        .detach();
    }
    pub(crate) fn speaker_group(&mut self, device: Device, leave: bool, cx: &mut Context<Self>) {
        let Some(coordinator) = self.cast_selected.clone() else {
            return;
        };
        if !leave {
            self.add_speaker_group(coordinator, vec![device], cx);
            return;
        }
        self.cancel_speaker_group();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cast_group_pending = Some(cancel.clone());
        let target = device.clone();
        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || Controller::new()?.leave(&target)).await;
            this.update(cx, |this, cx| {
                if this
                    .cast_group_pending
                    .as_ref()
                    .is_none_or(|pending| !Arc::ptr_eq(pending, &cancel))
                {
                    return;
                }
                this.cast_group_pending = None;
                if this.cast_selected.as_ref() != Some(&coordinator) {
                    return;
                }
                match result {
                    Ok(()) => {
                        this.cast_topology_generation += 1;
                        for group in &mut this.cast_groups {
                            group
                                .members
                                .retain(|member| !same_id(&member.id, &device.id));
                        }
                        this.cast_members.retain(|id, _| !same_id(id, &device.id));
                        this.cast_member_volumes
                            .retain(|id, _| !same_id(id, &device.id));
                        this.cast_member_mutes
                            .retain(|id, _| !same_id(id, &device.id));
                        this.cast_volume_pending
                            .retain(|id, _| !same_id(id, &device.id));
                        this.load_speaker_properties(coordinator, cx);
                    }
                    Err(error) => {
                        log::warn!("Speaker group removal failed: {error}");
                        this.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}
