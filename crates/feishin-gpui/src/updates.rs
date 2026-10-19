use crate::app::Feishin;
use crate::scroll::VerticalScroll;
use feishin_core::updates::{self, Available, Channel, Prepared};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, StyledExt,
    button::ButtonVariants,
    menu::{DropdownMenu, PopupMenuItem},
};
use std::{
    path::PathBuf,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

pub struct State {
    pub available: Option<Available>,
    pub ready: Option<Prepared>,
    pub downloaded: Option<PathBuf>,
    pub busy: bool,
    pub installing: bool,
    pub checked: bool,
    pub last_check: Instant,
    pub generation: u64,
    pub progress: (u64, u64),
    pub receiver: Option<Receiver<(u64, u64)>>,
    pub error: Option<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            available: None,
            ready: None,
            downloaded: None,
            busy: false,
            installing: false,
            checked: false,
            last_check: Instant::now(),
            generation: 0,
            progress: (0, 0),
            receiver: None,
            error: None,
        }
    }
}
impl Feishin {
    pub fn automatic_updates_enabled(&self) -> bool {
        self.state.settings.updates.automatic && !updates::disabled_by_environment()
    }
    pub fn poll_updates(&mut self, cx: &mut Context<Self>) {
        if let Some(receiver) = &self.updates.receiver {
            while let Ok(progress) = receiver.try_recv() {
                self.updates.progress = progress;
                cx.notify();
            }
        }
        if !cfg!(any(test, debug_assertions))
            && self.automatic_updates_enabled()
            && !self.updates.busy
            && self.updates.last_check.elapsed()
                >= if self.updates.checked {
                    Duration::from_secs(24 * 60 * 60)
                } else {
                    Duration::from_secs(15)
                }
        {
            self.check_updates(cx);
        }
    }
    pub fn check_updates(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy {
            return;
        }
        self.updates.busy = true;
        self.updates.checked = true;
        self.updates.last_check = Instant::now();
        self.updates.error = None;
        self.updates.progress = (0, 0);
        self.updates.generation += 1;
        let generation = self.updates.generation;
        let config = self.state.settings.updates.clone();
        let task = cx
            .background_executor()
            .spawn(async move { updates::check(&config) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.updates.generation {
                    return;
                }
                this.updates.busy = false;
                match result {
                    Ok(available) => {
                        if this
                            .updates
                            .available
                            .as_ref()
                            .map(|update| &update.version)
                            != available.as_ref().map(|update| &update.version)
                        {
                            this.updates.ready = None;
                            this.updates.downloaded = None;
                        }
                        this.updates.available = available;
                        if this.automatic_updates_enabled()
                            && this.updates.available.is_some()
                            && this.updates.downloaded.is_none()
                        {
                            this.download_update(cx);
                        }
                    }
                    Err(error) => {
                        log::warn!("Native update check failed: {error}");
                        this.updates.error = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub fn download_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.busy {
            return;
        }
        let Some(update) = self.updates.available.clone() else {
            return;
        };
        let current = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                self.updates.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let root = self.data_dir.join("Updates");
        self.updates.busy = true;
        self.updates.error = None;
        self.updates.progress = (0, update.size);
        self.updates.generation += 1;
        let generation = self.updates.generation;
        let (sender, receiver) = std::sync::mpsc::sync_channel(4);
        self.updates.receiver = Some(receiver);
        let task = cx.background_executor().spawn(async move {
            let archive = updates::download(&update, &root, |count, total| {
                let _ = sender.try_send((count, total));
            })?;
            let prepared = updates::prepare(&archive, &current);
            anyhow::Ok((archive, prepared))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.updates.generation {
                    return;
                }
                this.updates.receiver = None;
                this.updates.busy = false;
                match result {
                    Ok((archive, prepared)) => {
                        this.updates.downloaded = Some(archive);
                        this.updates.progress.0 = this.updates.progress.1;
                        match prepared {
                            Ok(prepared) => this.updates.ready = Some(prepared),
                            Err(error) => {
                                log::warn!("Cannot prepare native update: {error}");
                                this.updates.error = Some(error.to_string());
                            }
                        }
                    }
                    Err(error) => {
                        log::warn!("Native update download failed: {error}");
                        this.updates.error = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub fn install_update(&mut self, restart: bool, cx: &mut Context<Self>) {
        let Some(prepared) = self.updates.ready.as_ref() else {
            return;
        };
        match updates::launch(prepared, restart) {
            Ok(()) => {
                self.updates.installing = true;
                self.updates.ready = None;
                self.save();
                cx.quit();
            }
            Err(error) => {
                log::warn!("Cannot launch native update installer: {error}");
                self.updates.error = Some(error.to_string());
                cx.notify();
            }
        }
    }
    pub fn install_update_on_quit(&mut self) {
        if !self.updates.installing
            && self.automatic_updates_enabled()
            && let Some(prepared) = self.updates.ready.take()
            && let Err(error) = updates::launch(&prepared, false)
        {
            log::error!("Cannot install native update on quit: {error}");
        }
    }
    pub fn update_settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let channel = self.state.settings.updates.channel;
        let channel_key = |channel| match channel {
            Channel::Latest => "setting.releaseChannel_optionLatest",
            Channel::Beta => "setting.releaseChannel_optionBeta",
            Channel::Alpha => "setting.releaseChannel_optionAlpha",
        };
        div()
            .id("native-updates")
            .debug_selector(|| "native-updates".into())
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .child(format!("Feishin {}", env!("CARGO_PKG_VERSION")))
            .child(
                crate::styles::button("view-release-notes", cx)
                    .debug_selector(|| "view-release-notes".into())
                    .label(self.tr("View release notes"))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_release_notes(window, cx)),
                    ),
            )
            .when(!updates::disabled_by_environment(), |div| {
                div.child(
                    gpui::div()
                        .v_flex()
                        .gap_2()
                        .child(self.tr("setting.releaseChannel"))
                        .child(
                            gpui::div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.tr("setting.releaseChannel_description")),
                        )
                        .child(
                            crate::styles::button("native-update-channel", cx)
                                .debug_selector(|| "native-update-channel".into())
                                .label(self.tr(channel_key(channel)))
                                .aria_label(self.tr("setting.releaseChannel"))
                                .disabled(self.updates.busy)
                                .dropdown_menu({
                                    let view = cx.weak_entity();
                                    move |mut menu, _, cx| {
                                        for choice in Channel::ALL {
                                            let view = view.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(crate::appearance::text(
                                                    channel_key(choice),
                                                    cx,
                                                ))
                                                .checked(channel == choice)
                                                .on_click(move |_, _, cx| {
                                                    view.update(cx, |this, cx| {
                                                        // A check may start after this menu opened.
                                                        if this.updates.busy
                                                            || updates::disabled_by_environment()
                                                        {
                                                            return;
                                                        }
                                                        this.state.settings.updates.channel =
                                                            choice;
                                                        this.updates.available = None;
                                                        this.updates.ready = None;
                                                        this.updates.downloaded = None;
                                                        this.updates.checked = false;
                                                        this.updates.last_check = Instant::now();
                                                        this.save();
                                                        cx.notify();
                                                    })
                                                    .ok();
                                                }),
                                            );
                                        }
                                        menu
                                    }
                                }),
                        ),
                )
                .child(
                    self.setting_checkbox(
                        "automatic-native-updates",
                        "setting.automaticUpdates",
                        self.state.settings.updates.automatic,
                        cx,
                    )
                    .debug_selector(|| "automatic-native-updates".into())
                    .on_click(cx.listener(|this, enabled: &bool, _, cx| {
                        if updates::disabled_by_environment() {
                            return;
                        }
                        this.state.settings.updates.automatic = *enabled;
                        this.save();
                        cx.notify();
                    })),
                )
            })
            .child(
                crate::styles::button("check-native-updates", cx)
                    .label(self.tr("Check for updates"))
                    .disabled(self.updates.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.check_updates(cx))),
            )
            .when(self.updates.busy, |div| {
                div.child(if self.updates.progress.1 > 0 {
                    format!(
                        "Downloading update: {:.0}%",
                        100. * self.updates.progress.0 as f64 / self.updates.progress.1 as f64
                    )
                } else {
                    "Checking for updates…".into()
                })
            })
            .when(
                self.updates.checked
                    && !self.updates.busy
                    && self.updates.available.is_none()
                    && self.updates.error.is_none(),
                |div| {
                    div.child(crate::appearance::text(
                        "No newer native release is available for this platform and channel.",
                        cx,
                    ))
                },
            )
            .when_some(self.updates.error.as_ref(), |div, error| {
                div.child(error.clone())
            })
            .when_some(self.updates.available.as_ref(), |div, update| {
                let page = update.page.clone();
                div.child(format!("Update available: {}", update.version))
                    .child(
                        crate::styles::button("native-release-page", cx)
                            .ghost()
                            .label(crate::appearance::text("Release notes", cx))
                            .on_click(move |_, _, cx| cx.open_url(&page)),
                    )
                    .child(
                        gpui::div()
                            .id("native-release-notes")
                            .max_h(px(240.))
                            .native_vertical_scroll()
                            .child(update.notes.clone()),
                    )
                    .child(
                        crate::styles::button("download-native-update", cx)
                            .label(crate::appearance::text("Download update", cx))
                            .disabled(self.updates.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.download_update(cx))),
                    )
            })
            .when_some(self.updates.downloaded.as_ref(), |div, path| {
                let path = path.clone();
                div.child(
                    crate::styles::button("reveal-native-update", cx)
                        .ghost()
                        .label(crate::appearance::text("Show downloaded update", cx))
                        .on_click(move |_, _, cx| cx.reveal_path(&path)),
                )
            })
            .when(self.updates.ready.is_some(), |div| {
                div.child(
                    crate::styles::button("install-native-update", cx)
                        .label(crate::appearance::text("Install and restart", cx))
                        .on_click(cx.listener(|this, _, _, cx| this.install_update(true, cx))),
                )
            })
            .into_any_element()
    }
}
