use crate::app::Feishin;
use feishin_core::dlna::{Controller, Device, Group};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    animation::CssTransition,
    slider::{SliderEvent, SliderState},
};
use std::{collections::HashMap, time::Duration};

pub(crate) struct MemberUpdate {
    pub device: Device,
    generation: u64,
    volume: Option<u8>,
    muted: Option<bool>,
}

fn same_id(a: &str, b: &str) -> bool {
    a.trim_start_matches("uuid:") == b.trim_start_matches("uuid:")
}

impl Feishin {
    pub(crate) fn player_volume_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.cast_selected.is_none() {
            return div()
                .h_flex()
                .gap(rems(0.25))
                .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                .child(self.volume_button(window, cx))
                .when(
                    f32::from(window.viewport_size().width) / self.state.settings.zoom > 480.,
                    |row| row.child(self.volume_slider(window, cx)),
                )
                .into_any_element();
        }
        let app = cx.weak_entity();
        window
            .use_keyed_state("speaker-volume-controls", cx, |_, cx| {
                SpeakerVolume::new(app, cx)
            })
            .into_any_element()
    }
    pub(crate) fn speaker_volume_group(&self) -> Option<&Group> {
        let selected = self.cast_selected.as_ref()?;
        self.cast_groups.iter().find(|group| {
            !group.stereo_pair
                && group
                    .members
                    .iter()
                    .filter(|member| !member.satellite)
                    .count()
                    > 1
                && same_id(&group.coordinator, &selected.id)
        })
    }

    fn speaker_volume_members(&self) -> Vec<String> {
        self.speaker_volume_group().map_or_else(Vec::new, |group| {
            group
                .members
                .iter()
                .filter(|member| !member.satellite && !same_id(&member.id, &group.coordinator))
                .map(|member| member.id.clone())
                .collect()
        })
    }

    pub(crate) fn change_master_volume(&mut self, value: f32, shift: bool, cx: &mut Context<Self>) {
        if !shift {
            for id in self.speaker_volume_members() {
                self.speaker_member_volume(id, value.clamp(0., 100.) as u8, cx);
            }
        }
        self.change_volume_slider(value, cx);
    }

    pub(crate) fn toggle_volume_mute(&mut self, shift: bool, cx: &mut Context<Self>) {
        let muted = !self.state.settings.muted;
        if !shift {
            for id in self.speaker_volume_members() {
                self.speaker_member_mute(id, muted, cx);
            }
        }
        self.state.settings.muted = muted;
        self.apply_audio_settings(cx);
    }

    pub(crate) fn change_member_volume(
        &mut self,
        id: String,
        volume: u8,
        slider: bool,
        cx: &mut Context<Self>,
    ) {
        let muted = self.cast_member_mutes.get(&id).copied().unwrap_or(false);
        self.speaker_member_volume(id.clone(), volume, cx);
        if (muted && volume > 0) || (!muted && volume == 0 && slider) {
            self.speaker_member_mute(id, volume == 0, cx);
        }
    }

    pub(crate) fn speaker_member_volume(&mut self, id: String, volume: u8, cx: &mut Context<Self>) {
        self.queue_member_update(id, Some(volume.min(100)), None, cx);
    }

    pub(crate) fn speaker_member_mute(&mut self, id: String, muted: bool, cx: &mut Context<Self>) {
        self.queue_member_update(id, None, Some(muted), cx);
    }

    fn queue_member_update(
        &mut self,
        id: String,
        volume: Option<u8>,
        muted: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        let Some(device) = self.cast_members.get(&id).cloned() else {
            return;
        };
        // Invalidate reads already in flight and keep only the latest requested values.
        self.cast_topology_generation += 1;
        if let Some(volume) = volume {
            self.cast_member_volumes.insert(id.clone(), volume);
        }
        if let Some(muted) = muted {
            self.cast_member_mutes.insert(id.clone(), muted);
        }
        cx.notify();
        if let Some(pending) = self
            .cast_volume_pending
            .get_mut(&id)
            .filter(|pending| pending.device == device)
        {
            pending.volume = volume.or(pending.volume);
            pending.muted = muted.or(pending.muted);
            return;
        }
        let generation = self.cast_topology_generation;
        self.cast_volume_pending.insert(
            id.clone(),
            MemberUpdate {
                device: device.clone(),
                generation,
                volume,
                muted,
            },
        );
        cx.spawn(async move |this, cx| {
            loop {
                let Some((volume, muted)) = this
                    .update(cx, |this, _| {
                        let pending = this.cast_volume_pending.get_mut(&id)?;
                        if pending.generation != generation {
                            return None;
                        }
                        if this.cast_members.get(&id) != Some(&device)
                            || (pending.volume.is_none() && pending.muted.is_none())
                        {
                            this.cast_volume_pending.remove(&id);
                            return None;
                        }
                        Some((pending.volume.take(), pending.muted.take()))
                    })
                    .ok()
                    .flatten()
                else {
                    break;
                };
                let device = device.clone();
                let result = smol::unblock(move || {
                    let controller = Controller::new()?;
                    if let Some(volume) = volume {
                        controller.volume(&device, volume)?;
                    }
                    if let Some(muted) = muted {
                        controller.mute(&device, muted)?;
                    }
                    Ok::<_, anyhow::Error>(())
                })
                .await;
                if let Err(error) = result {
                    log::warn!("Speaker member update failed: {error}");
                    this.update(cx, |this, cx| {
                        if this
                            .cast_volume_pending
                            .get(&id)
                            .is_some_and(|pending| pending.generation == generation)
                        {
                            this.message = error.to_string();
                            cx.notify();
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

struct SpeakerVolume {
    app: WeakEntity<Feishin>,
    members: HashMap<String, (Entity<SliderState>, Subscription)>,
    focus: FocusHandle,
    hovered: bool,
    panel_hovered: bool,
    visibility: CssTransition,
    member_opacity: CssTransition,
}
impl SpeakerVolume {
    fn new(app: WeakEntity<Feishin>, cx: &mut Context<Self>) -> Self {
        Self {
            app,
            members: HashMap::new(),
            focus: cx.focus_handle(),
            hovered: false,
            panel_hovered: false,
            visibility: CssTransition::new(0., Duration::from_millis(180), [0.25, 0.1, 0.25, 1.]),
            member_opacity: CssTransition::new(
                0.4,
                Duration::from_millis(200),
                [0.25, 0.1, 0.25, 1.],
            ),
        }
    }
}
impl Render for SpeakerVolume {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = self.app.upgrade() else {
            return div().into_any_element();
        };
        let (group, volumes, mutes, width, zoom, step) = app.read_with(cx, |app, _| {
            (
                app.speaker_volume_group().cloned(),
                app.cast_member_volumes.clone(),
                app.cast_member_mutes.clone(),
                app.state.settings.volume_width,
                app.state.settings.zoom,
                app.state.settings.volume_wheel_step,
            )
        });
        let rtl = crate::appearance::rtl(cx);
        let small = f32::from(window.viewport_size().width) / zoom <= 480.;
        let shift = window.modifiers().shift;
        let visible = self.hovered || self.panel_hovered || self.focus.contains_focused(window, cx);
        let now = cx.background_executor().now();
        self.visibility.retarget(f32::from(visible), now);
        self.member_opacity
            .retarget(if shift { 1. } else { 0.4 }, now);
        let (opacity, animating) = self.visibility.value(now);
        let (member_opacity, member_animating) = self.member_opacity.value(now);
        if animating || member_animating {
            window.request_animation_frame();
        }
        self.members.retain(|id, _| {
            group
                .as_ref()
                .is_some_and(|g| g.members.iter().any(|member| &member.id == id))
        });
        let mut root = div()
            .id("network-volume")
            .debug_selector(|| "network-volume".into())
            .role(Role::GenericContainer)
            .relative()
            .flex_shrink_0()
            .track_focus(&self.focus)
            .on_modifiers_changed(cx.listener(|_, _, _, cx| cx.notify()))
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .on_click(|_, _, cx| cx.stop_propagation());
        if let Some(group) = group {
            let mut panel = div()
                .id("network-volume-panel")
                .debug_selector(|| "network-volume-panel".into())
                .v_flex()
                .absolute()
                .left(rems(-0.5))
                .right(rems(-0.5))
                .bottom(rems(-0.5 - (1. - opacity) * 0.25))
                .px(rems(0.5))
                .pb(rems(2.25))
                .border(px(zoom))
                .border_color(cx.theme().border)
                .rounded(rems(5. / 16.))
                .bg(cx.theme().background)
                .shadow(
                    [(20., 25., 0.1), (10., 10., 0.04)]
                        .map(|(y, blur, alpha)| BoxShadow {
                            color: black().opacity(alpha),
                            offset: point(px(0.), px(y * zoom)),
                            blur_radius: px(blur * zoom),
                            spread_radius: px(0.),
                        })
                        .to_vec(),
                )
                .text_color(cx.theme().foreground)
                .opacity(opacity)
                .when(!visible, |panel| panel.mouse_passthrough_subtree())
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.panel_hovered = *hovered;
                    cx.notify();
                }))
                .child(
                    div()
                        .py(rems(0.25))
                        .text_size(rems(11. / 16.))
                        .line_height(rems(0.875))
                        .text_center()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::appearance::text(
                            if shift {
                                "dlna.volume.individualControl"
                            } else {
                                "dlna.volume.groupControl"
                            },
                            cx,
                        )),
                )
                .child(
                    div()
                        .mx(rems(-0.5))
                        .mb(rems(0.5))
                        .border_b(px(zoom))
                        .border_color(cx.theme().border),
                );
            for (index, member) in group
                .members
                .iter()
                .filter(|member| !member.satellite && !same_id(&member.id, &group.coordinator))
                .enumerate()
            {
                let id = member.id.clone();
                let volume = volumes.get(&id).copied().unwrap_or(0);
                let muted = mutes.get(&id).copied().unwrap_or(false);
                let (slider, _) = self.members.entry(id.clone()).or_insert_with(|| {
                    let slider = cx.new(|_| {
                        SliderState::new()
                            .min(0.)
                            .max(100.)
                            .step(1.)
                            .default_value(volume as f32)
                    });
                    let id = id.clone();
                    let subscription = app.update(cx, |_, cx| {
                        cx.subscribe_in(&slider, window, move |app, _, event, window, cx| {
                            if let SliderEvent::Change(value) = event
                                && window.modifiers().shift
                            {
                                app.change_member_volume(id.clone(), value.end() as u8, true, cx);
                            }
                        })
                    });
                    (slider, subscription)
                });
                if slider.read(cx).value().end() != volume as f32 {
                    slider.update(cx, |slider, cx| slider.set_value(volume as f32, window, cx));
                }
                let slider = slider.clone();
                let mute_app = app.clone();
                let mute_id = id.clone();
                let wheel_app = app.clone();
                let wheel_id = id.clone();
                let controls = div()
                    .id(("member-volume-controls", index))
                    .debug_selector(move || format!("network-member-controls-{index}"))
                    .h_flex()
                    .gap(rems(0.25))
                    .when(rtl, |row| row.flex_row_reverse())
                    .on_scroll_wheel(move |event, _, cx| {
                        if !event.modifiers.shift {
                            return;
                        }
                        let delta = event.delta.pixel_delta(px(1.));
                        if delta.x == px(0.) && delta.y == px(0.) {
                            return;
                        }
                        wheel_app.update(cx, |app, cx| {
                            let current =
                                app.cast_member_volumes.get(&wheel_id).copied().unwrap_or(0) as f32;
                            let value = (current
                                + if delta.x < px(0.) || delta.y < px(0.) {
                                    -step
                                } else {
                                    step
                                })
                            .clamp(0., 100.);
                            app.change_member_volume(wheel_id.clone(), value as u8, false, cx);
                        });
                        cx.stop_propagation();
                    })
                    .child(
                        crate::playerbar::button("network-member-mute", false, window, cx)
                            .debug_selector(move || format!("network-member-mute-{index}"))
                            .disabled(!shift)
                            .text_color(cx.theme().foreground)
                            .icon(
                                Icon::default()
                                    .path(if muted {
                                        "native-icons/volume-muted.svg"
                                    } else if volume > 50 {
                                        "native-icons/volume-high.svg"
                                    } else {
                                        "native-icons/volume-low.svg"
                                    })
                                    .with_size(window.rem_size() * 1.125),
                            )
                            .tooltip(format!(
                                "{}: {}",
                                member.name,
                                if muted {
                                    crate::appearance::text("player.muted", cx)
                                } else {
                                    volume.to_string()
                                }
                            ))
                            .on_click(move |event, _, cx| {
                                if event.modifiers().shift {
                                    mute_app.update(cx, |app, cx| {
                                        app.speaker_member_mute(mute_id.clone(), !muted, cx)
                                    });
                                }
                                cx.stop_propagation();
                            }),
                    )
                    .when(!small, |row| {
                        row.child(
                            div()
                                .w(rems(width.clamp(0., 180.) / 16.))
                                .min_w(rems(0.75))
                                .flex_shrink_0()
                                .child(
                                    crate::styles::slider(&slider, cx)
                                        .playerbar(zoom, "network-member-volume")
                                        .disabled(!shift)
                                        .aria_label(member.name.clone())
                                        .value_label(volume.to_string()),
                                ),
                        )
                    });
                panel = panel.child(
                    div()
                        .debug_selector(move || format!("network-member-{index}"))
                        .v_flex()
                        .mb(rems(0.5))
                        .opacity(member_opacity)
                        .child(
                            div()
                                .mb(rems(0.25))
                                .text_size(rems(11. / 16.))
                                .line_height(rems(0.875))
                                .truncate()
                                .child(member.name.clone()),
                        )
                        .child(controls),
                );
            }
            let coordinator = group
                .members
                .iter()
                .find(|member| same_id(&member.id, &group.coordinator));
            panel = panel.child(
                div()
                    .mb(rems(0.25))
                    .text_size(rems(11. / 16.))
                    .line_height(rems(0.875))
                    .text_color(cx.theme().primary)
                    .truncate()
                    .child(coordinator.map_or_else(String::new, |member| member.name.clone())),
            );
            root = root.child(deferred(panel).with_priority(200));
        }
        root.child(
            deferred(app.update(cx, |app, cx| {
                div()
                    .id("network-volume-master")
                    .debug_selector(|| "network-volume-master".into())
                    .h_flex()
                    .gap(rems(0.25))
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(app.volume_button(window, cx))
                    .when(!small, |row| row.child(app.volume_slider(window, cx)))
            }))
            .with_priority(201),
        )
        .into_any_element()
    }
}

#[cfg(all(test, unix))]
#[path = "speaker_volume_tests.rs"]
mod tests;
