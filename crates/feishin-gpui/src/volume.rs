use crate::app::Feishin;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
};

impl Feishin {
    pub(crate) fn volume_slider(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if f32::from(window.viewport_size().width) / self.state.settings.zoom <= 480. {
            return div().into_any_element();
        }
        div()
            .debug_selector(|| "player-volume".into())
            .w(rems(self.state.settings.volume_width.clamp(0., 180.) / 16.))
            .min_w(rems(0.75))
            .flex_shrink_0()
            .on_scroll_wheel(cx.listener(Self::volume_wheel))
            .child(
                crate::styles::slider(&self.volume, cx)
                    .playerbar(self.state.settings.zoom, "player-volume")
                    .aria_label(self.tr("Volume"))
                    .value_label(self.volume.read(cx).value().end().to_string()),
            )
            .into_any_element()
    }

    fn commit_slider_volume(&mut self, value: f32, cx: &mut Context<Self>) {
        self.state.settings.volume = value;
        self.state.settings.muted = false;
        self.player
            .send(feishin_core::playback::Control::Volume(value));
        self.save();
        cx.notify();
    }

    pub(crate) fn change_volume_slider(&mut self, value: f32, cx: &mut Context<Self>) {
        if self.volume_change_task.is_some() {
            return;
        }
        self.commit_slider_volume(value, cx);
        self.volume_change_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                if !this
                    .update(cx, |this, cx| {
                        // Read the current thumb so an external adjustment wins over
                        // a queued pointer value. The first change remains immediate.
                        let value = this.volume.read(cx).value().end();
                        if value == this.state.settings.volume {
                            this.volume_change_task = None;
                            false
                        } else {
                            this.commit_slider_volume(value, cx);
                            true
                        }
                    })
                    .unwrap_or(false)
                {
                    break;
                }
            }
        }));
    }

    pub fn volume_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(px(1.));
        if delta.x == px(0.) && delta.y == px(0.) {
            return;
        }
        if self.cast_selected.is_some() {
            let step = self.state.settings.volume_wheel_step.clamp(1., 20.);
            let value = (self.volume.read(cx).value().end()
                + if delta.x < px(0.) || delta.y < px(0.) {
                    -step
                } else {
                    step
                })
            .clamp(0., 100.);
            self.volume
                .update(cx, |slider, cx| slider.set_value(value, window, cx));
            self.change_master_volume(value, event.modifiers.shift, cx);
            cx.stop_propagation();
            return;
        }
        self.run_shortcut(
            if delta.x < px(0.) || delta.y < px(0.) {
                feishin_core::shortcuts::Action::VolumeDown
            } else {
                feishin_core::shortcuts::Action::VolumeUp
            },
            window,
            cx,
        );
        cx.stop_propagation();
    }

    pub fn volume_button(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let muted = self.state.settings.muted;
        let control = div()
            .id("player-mute")
            .debug_selector(|| "player-mute".into())
            .on_scroll_wheel(cx.listener(Self::volume_wheel))
            .child(
                crate::playerbar::button("mute", false, window, cx)
                    .when(muted, |button| {
                        button.text_color(cx.theme().muted_foreground)
                    })
                    .icon(
                        Icon::default()
                            .path(if muted {
                                "native-icons/volume-muted.svg"
                            } else if self.state.settings.volume > 50. {
                                "native-icons/volume-high.svg"
                            } else {
                                "native-icons/volume-low.svg"
                            })
                            .with_size(window.rem_size() * 1.125),
                    )
                    .tooltip(if muted {
                        self.tr("player.muted")
                    } else {
                        format!("{:.0}", self.state.settings.volume)
                    })
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.toggle_volume_mute(event.modifiers().shift, cx);
                        cx.stop_propagation();
                    })),
            );
        if self.cast_selected.is_some() || self.state.settings.jukebox {
            return control.into_any_element();
        }
        control
            .context_menu(self.volume_device_menu(cx))
            .into_any_element()
    }

    fn volume_device_menu(
        &self,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let app = cx.weak_entity();
        move |menu, _, cx| {
            let Ok((settings, default_label)) = app.read_with(cx, |app, _| {
                (
                    app.state.settings.clone(),
                    app.tr("setting.audioDeviceDefault"),
                )
            }) else {
                return menu;
            };
            let source = settings.mpv_path.clone();
            let default = if source.is_empty() { "" } else { "auto" };
            let current = if source.is_empty() {
                settings.audio_device.clone()
            } else {
                settings.mpv_audio_device.clone()
            };
            let item = device_menu_item(
                default.to_owned(),
                default_label,
                current == default,
                source.clone(),
                app.clone(),
            );
            let app = app.clone();
            cx.spawn(async move |menu, cx| {
                let result =
                    smol::unblock(move || feishin_core::playback::output_devices(&settings)).await;
                if app
                    .read_with(cx, |app, _| {
                        app.state.settings.mpv_path != source
                            || app.cast_selected.is_some()
                            || app.state.settings.jukebox
                    })
                    .unwrap_or(true)
                {
                    return;
                }
                menu.update(cx, |menu, cx| {
                    let items = match result {
                        Ok(devices) => devices
                            .into_iter()
                            .filter(|(id, _)| id != default)
                            .map(|(id, label)| {
                                let selected = id == current;
                                device_menu_item(id, label, selected, source.clone(), app.clone())
                            })
                            .collect::<Vec<_>>(),
                        Err(error) => {
                            log::warn!("Volume device query failed: {error}");
                            vec![PopupMenuItem::new(error.to_string()).disabled(true)]
                        }
                    };
                    if !items.is_empty() {
                        menu.extend_items(
                            std::iter::once(PopupMenuItem::separator()).chain(items),
                            cx,
                        );
                    }
                })
                .ok();
            })
            .detach();
            menu.item(item)
        }
    }
}

fn device_menu_item(
    id: String,
    label: String,
    selected: bool,
    source: String,
    app: WeakEntity<Feishin>,
) -> PopupMenuItem {
    PopupMenuItem::new(label)
        .checked(selected)
        .on_click(move |_, window, cx| {
            let (app, id, source) = (app.clone(), id.clone(), source.clone());
            window.defer(cx, move |window, cx| {
                app.update(cx, |app, cx| {
                    if app.state.settings.mpv_path == source
                        && app.cast_selected.is_none()
                        && !app.state.settings.jukebox
                    {
                        app.select_playback_device(id, window, cx);
                    }
                })
                .ok();
            });
        })
}

#[cfg(all(test, unix))]
mod tests {
    use crate::app::{Feishin, Route};
    use feishin_core::{model::Item, queue::Enqueue, storage::State};
    use gpui::{
        AppContext, Modifiers, MouseButton, Role, ScrollDelta, ScrollWheelEvent, TestAppContext,
        TouchPhase, VisualTestContext, point, px,
    };
    use gpui_component::Root;
    use std::{
        cell::RefCell,
        os::unix::fs::PermissionsExt,
        rc::Rc,
        time::{Duration, Instant},
    };

    fn menu_item(cx: &mut VisualTestContext, label: &str) -> Option<gpui::Point<gpui::Pixels>> {
        cx.update(|window, _| {
            let tree = window.accessibility_tree_for_test()?;
            let consumer = accesskit_consumer::Tree::new(tree.clone(), true);
            let (_, node) = tree.nodes.iter().find(|(id, node)| {
                node.role() == Role::MenuItem
                    && consumer
                        .state()
                        .node_by_tree_local_id(*id, gpui::accesskit::TreeId::ROOT)
                        .unwrap()
                        .label()
                        .as_deref()
                        == Some(label)
            })?;
            let bounds = node.bounds()?;
            let scale = f64::from(window.scale_factor());
            Some(point(
                px(((bounds.x0 + bounds.x1) / (2. * scale)) as f32),
                px(((bounds.y0 + bounds.y1) / (2. * scale)) as f32),
            ))
        })
    }
    fn open_menu(cx: &mut VisualTestContext) {
        let point = cx.debug_bounds("player-mute").unwrap().center();
        cx.simulate_mouse_down(point, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(point, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
    }
    fn wait(cx: &mut VisualTestContext, condition: impl Fn(&mut VisualTestContext) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            if condition(cx) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Volume device response did not settle"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[gpui::test]
    fn volume_mouse_wheel_and_device_menu_preserve_playback_and_reject_stale_choices(
        cx: &mut TestAppContext,
    ) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-volume-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("mpv-fixture");
        let log = dir.join("commands.jsonl");
        std::fs::write(
            &binary,
            include_str!("../../../scripts/native-tests/mpv-device-fixture.py"),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = binary.to_string_lossy().to_string();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.fade_on_status = false;
            state.settings.volume_wheel_step = 7.;
            state
                .settings
                .shortcuts
                .get_mut(&feishin_core::shortcuts::Action::VolumeUp)
                .unwrap()
                .key = "alt-up".into();
            state.settings.audio_device = "Native DAC".into();
            state.settings.mpv_path = path.clone();
            state.settings.mpv_extra_args = vec![format!("--log-file={}", log.display())];
            state.queue.add(
                vec![Item {
                    id: "song".into(),
                    name: "Song".into(),
                    ..Default::default()
                }],
                Enqueue::Now,
            );
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(gpui::size(px(1400.), px(1000.)));
        cx.run_until_parked();
        let original = app.read_with(cx, |app, _| {
            (
                app.route.clone(),
                app.history.clone(),
                serde_json::to_value(&app.state.queue).unwrap(),
            )
        });
        for selector in ["player-mute", "player-volume"] {
            for (x, y, initial, expected) in [
                (0., 1., 50., 57.),
                (1., 0., 50., 57.),
                (-1., 0., 50., 43.),
                (-1., 1., 50., 43.),
                (0., -1., 3., 0.),
                (0., 1., 99., 100.),
                (0., 0., 50., 50.),
            ] {
                cx.update(|window, cx| {
                    app.update(cx, |app, cx| {
                        app.state.settings.volume = initial;
                        app.state.settings.muted = true;
                        app.volume
                            .update(cx, |slider, cx| slider.set_value(initial, window, cx));
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                let position = cx.debug_bounds(selector).unwrap().center();
                cx.simulate_event(ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Lines(point(x, y)),
                    modifiers: Modifiers::none(),
                    touch_phase: TouchPhase::Moved,
                });
                cx.run_until_parked();
                app.read_with(cx, |app, cx| {
                    assert_eq!(app.state.settings.volume, expected, "{selector}: {x},{y}");
                    assert_eq!(app.volume.read(cx).value().end(), expected);
                    assert_eq!(app.state.settings.muted, x == 0. && y == 0.);
                    assert_eq!(app.route, original.0);
                    assert_eq!(app.history, original.1);
                    assert_eq!(serde_json::to_value(&app.state.queue).unwrap(), original.2);
                });
            }
        }
        let point = cx.debug_bounds("player-mute").unwrap().center();
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        assert!(!app.read_with(cx, |app, _| app.state.settings.muted));
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.playing = true;
                cx.notify();
            })
        });
        cx.run_until_parked();
        open_menu(cx);
        assert!(!app.read_with(cx, |app, _| app.state.settings.muted));
        wait(cx, |cx| menu_item(cx, "Studio DAC").is_some());
        let point = menu_item(cx, "Studio DAC").unwrap();
        cx.simulate_event(ScrollWheelEvent {
            position: point,
            delta: ScrollDelta::Lines(gpui::point(0., 1.)),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |app, _| app.state.settings.volume),
            50.,
            "Menu scrolling must not change playback volume"
        );
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |app, cx| {
            assert!(app.playing);
            assert_eq!(app.state.settings.mpv_audio_device, "coreaudio/42");
            assert_eq!(app.mpv_device.read(cx).value(), "coreaudio/42");
            assert_eq!(app.state.settings.audio_device, "Native DAC");
            assert_eq!(app.route, original.0);
            assert_eq!(serde_json::to_value(&app.state.queue).unwrap(), original.2);
        });
        assert_eq!(
            feishin_core::storage::load(&dir.join("state.json"))
                .unwrap()
                .settings
                .mpv_audio_device,
            "coreaudio/42"
        );
        cx.simulate_keystrokes("alt-up");
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |app, _| app.state.settings.volume),
            57.,
            "Shortcuts remain available after dismissing the device menu"
        );
        open_menu(cx);
        wait(cx, |cx| menu_item(cx, "null").is_some());
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.settings.mpv_path.clear();
                cx.notify();
            })
        });
        cx.run_until_parked();
        let point = menu_item(cx, "null").unwrap();
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(app.state.settings.audio_device, "Native DAC");
            assert_eq!(app.state.settings.mpv_audio_device, "coreaudio/42");
        });
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.settings.mpv_path = path.clone();
                app.navigate(Route::Outputs, cx);
            })
        });
        cx.run_until_parked();
        let point = cx.debug_bounds("refresh-audio-devices").unwrap().center();
        cx.simulate_click(point, Modifiers::none());
        wait(cx, |cx| {
            app.read_with(cx, |app, _| {
                app.audio_devices
                    .iter()
                    .any(|(id, label)| id == "coreaudio/42" && label == "Studio DAC")
            })
        });
        assert!(app.read_with(cx, |app, _| app.playing));
        assert!(!std::fs::read_to_string(log).unwrap().contains("loadfile"));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[gpui::test]
    fn playerbar_volume_sizes_throttling_and_album_modifiers_match_original(
        cx: &mut TestAppContext,
    ) {
        use gpui::{prelude::*, *};
        use gpui_component::{
            Theme,
            slider::{SliderEvent, SliderValue},
        };
        use std::cell::Cell;
        struct Probe {
            app: Entity<Feishin>,
            slider_only: bool,
        }
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                if self.slider_only {
                    div()
                        .w_full()
                        .child(self.app.update(cx, |app, cx| app.volume_slider(window, cx)))
                        .into_any_element()
                } else {
                    self.app.clone().into_any_element()
                }
            }
        }
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-volume-controls-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.scrobble = false;
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|_| Probe {
                app: app.clone(),
                slider_only: true,
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        let controls: serde_json::Value =
            serde_json::from_str(include_str!("../assets/playerbar-control-reference.json"))
                .unwrap();
        let layout = crate::progress::layout_reference();
        for case in layout["volumeCases"].as_array().unwrap() {
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let rem = case["rem"].as_f64().unwrap() as f32;
            cx.simulate_resize(size(px(1200. * zoom), px(600.)));
            cx.update(|window, cx| {
                window.set_rem_size(px(rem * zoom));
                Theme::global_mut(cx).font_size = px(rem * zoom);
                cx.set_global(crate::appearance::Language(
                    if case["rtl"] == true { "ar" } else { "en" }.into(),
                ));
                app.update(cx, |app, cx| {
                    app.state.settings.zoom = zoom;
                    app.state.settings.volume_width = case["width"].as_f64().unwrap() as f32;
                    app.volume
                        .update(cx, |slider, cx| slider.set_value(50., window, cx));
                });
                probe.update(cx, |_, cx| cx.notify());
            });
            cx.run_until_parked();
            for (name, selector) in [
                ("root", "player-volume-slider"),
                ("bar", "player-volume-fill"),
                ("thumb", "player-volume-thumb"),
            ] {
                let bounds = cx.debug_bounds(selector).unwrap();
                for (actual, expected) in
                    [f32::from(bounds.size.width), f32::from(bounds.size.height)]
                        .into_iter()
                        .zip(case["elements"][name].as_array().unwrap())
                {
                    assert!(
                        (actual - expected.as_f64().unwrap() as f32 * zoom).abs() < 1.1,
                        "{name}: {actual}, {case}"
                    );
                }
            }
        }
        for case in controls["volume"].as_array().unwrap() {
            cx.simulate_resize(size(
                px(case["viewport"].as_f64().unwrap() as f32),
                px(600.),
            ));
            cx.update(|window, cx| {
                window.set_rem_size(px(16.));
                Theme::global_mut(cx).font_size = px(16.);
                app.update(cx, |app, _| {
                    app.state.settings.zoom = 1.;
                    app.state.settings.volume_width = case["width"].as_f64().unwrap() as f32;
                });
                probe.update(cx, |_, cx| cx.notify());
            });
            cx.run_until_parked();
            let visible = cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.role() == Role::Slider)
            });
            assert_eq!(visible, case["visible"].as_bool().unwrap());
        }
        for scenario in controls["throttle"].as_array().unwrap() {
            cx.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.volume_change_task = None;
                    app.state.settings.volume = 50.;
                    app.volume
                        .update(cx, |slider, cx| slider.set_value(50., window, cx));
                })
            });
            cx.run_until_parked();
            let start = cx.update(|_, cx| cx.background_executor().now());
            let calls = Rc::new(RefCell::new(Vec::new()));
            let previous = Rc::new(Cell::new(50.));
            let _watch = cx.update(|_, cx| {
                cx.observe(&app, {
                    let calls = calls.clone();
                    let previous = previous.clone();
                    move |app, cx| {
                        let value = app.read(cx).state.settings.volume;
                        if previous.replace(value) != value {
                            calls.borrow_mut().push((
                                cx.background_executor()
                                    .now()
                                    .duration_since(start)
                                    .as_millis() as u64,
                                value,
                            ));
                        }
                    }
                })
            });
            let mut elapsed = 0;
            for frame in scenario["frames"].as_array().unwrap() {
                let time = frame["clock"].as_u64().unwrap();
                while elapsed < time {
                    cx.executor().advance_clock(Duration::from_millis(1));
                    elapsed += 1;
                    cx.run_until_parked();
                }
                let value = frame["value"].as_f64().unwrap() as f32;
                cx.update(|window, cx| {
                    app.update(cx, |app, cx| {
                        app.volume.update(cx, |slider, cx| {
                            slider.set_value(value, window, cx);
                            cx.emit(SliderEvent::Change(SliderValue::Single(value)));
                        })
                    })
                });
                cx.run_until_parked();
                let expected = frame["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|call| {
                        (
                            call["clock"].as_u64().unwrap(),
                            call["value"].as_f64().unwrap() as f32,
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(*calls.borrow(), expected);
                assert_eq!(
                    app.read_with(cx, |app, cx| app.volume.read(cx).value().end()),
                    value
                );
            }
            // Advance through every deadline so scheduled trailing calls keep their cadence.
            while elapsed < 500 {
                let step = (500 - elapsed).min(1);
                cx.executor().advance_clock(Duration::from_millis(step));
                elapsed += step;
                cx.run_until_parked();
            }
            let expected = scenario["calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|call| {
                    (
                        call["clock"].as_u64().unwrap(),
                        call["value"].as_f64().unwrap() as f32,
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(*calls.borrow(), expected);
        }
        // A keyboard adjustment while a pointer update waits must remain authoritative.
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.volume.update(cx, |slider, cx| {
                    slider.set_value(30., window, cx);
                    cx.emit(SliderEvent::Change(SliderValue::Single(30.)));
                });
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.volume.update(cx, |slider, cx| {
                    slider.set_value(90., window, cx);
                    cx.emit(SliderEvent::Change(SliderValue::Single(90.)));
                });
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.run_shortcut(feishin_core::shortcuts::Action::VolumeDown, window, cx)
            })
        });
        let expected = app.read_with(cx, |app, _| app.state.settings.volume);
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |app, _| app.state.settings.volume),
            expected
        );

        cx.simulate_resize(size(px(1600.), px(900.)));
        cx.update(|window, cx| {
            cx.set_global(crate::appearance::Language("en".into()));
            app.update(cx, |app, cx| {
                app.state.queue.add(
                    (0..6)
                        .map(|i| Item {
                            id: format!("song-{i}"),
                            album_id: format!("album-{}", i / 2),
                            kind: feishin_core::model::ItemKind::Song,
                            duration: 100.,
                            ..Default::default()
                        })
                        .collect(),
                    Enqueue::Now,
                );
                app.state.settings.playerbar_open_drawer = true;
                app.apply_theme(window, cx);
            });
            probe.update(cx, |probe, cx| {
                probe.slider_only = false;
                cx.notify();
            });
        });
        cx.run_until_parked();
        for case in controls["transport"].as_array().unwrap() {
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.state.queue.current = Some(app.state.queue.entries[2].key);
                    app.state.queue.position = 0.;
                    app.loaded = false;
                    app.playing = false;
                    cx.notify();
                })
            });
            cx.run_until_parked();
            let next = case["call"][0] == "next";
            let alt = case["altKey"].as_bool().unwrap();
            let point = cx
                .debug_bounds(if next {
                    "player-next"
                } else {
                    "player-previous"
                })
                .unwrap()
                .center();
            cx.simulate_click(
                point,
                Modifiers {
                    alt,
                    ..Default::default()
                },
            );
            cx.run_until_parked();
            let index = match (next, alt) {
                (true, true) => 4,
                (true, false) => 3,
                (false, true) => 0,
                (false, false) => 1,
            };
            app.read_with(cx, |app, _| {
                assert_eq!(
                    app.state.queue.current,
                    Some(app.state.queue.entries[index].key)
                );
                assert!(!app.full_player);
            });
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
