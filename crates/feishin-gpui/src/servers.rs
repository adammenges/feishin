use crate::app::Feishin;
use feishin_core::{
    api::Api,
    model::{ItemKind, ListQuery, Server, ServerOptions, ServerType},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, StyledExt,
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
};
use uuid::Uuid;

#[derive(Default)]
pub struct SessionCheck {
    generation: u64,
    pub busy: Option<Uuid>,
}
impl SessionCheck {
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.busy = None;
    }
}

pub enum DiscoveryEvent {
    Found(feishin_core::server_discovery::Server),
    Finished(Result<(), String>),
}
#[derive(Default)]
pub struct Discovery {
    pub servers: Vec<feishin_core::server_discovery::Server>,
    pub receiver: Option<std::sync::mpsc::Receiver<DiscoveryEvent>>,
    pub cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub started: bool,
    pub busy: bool,
    pub error: Option<String>,
}
impl Drop for Discovery {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Feishin {
    pub(crate) fn server_input(
        &self,
        label: &'static str,
        input: gpui_component::input::Input,
    ) -> Div {
        div()
            .v_flex()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.tr(label)),
            )
            .child(input)
    }

    pub(crate) fn restore_server_sessions(
        &mut self,
        sessions: Vec<(Server, feishin_core::model::Session)>,
        cx: &mut Context<Self>,
    ) {
        let mut restored_active = false;
        for (saved, session) in sessions {
            if self.state.signed_out_servers.contains(&saved.id)
                || self.sessions.contains_key(&saved.id)
            {
                continue;
            }
            let Some(server) = self
                .state
                .servers
                .iter()
                .find(|server| feishin_core::storage::same_account(server, &saved))
            else {
                continue;
            };
            let api = match Api::new(server.clone(), session) {
                Ok(api) => api,
                Err(error) => {
                    log::warn!("Could not restore server account {}: {error}", saved.id);
                    continue;
                }
            };
            api.enable_session_persistence();
            restored_active |= self.state.active_server == Some(server.id);
            if self.state.active_server != Some(server.id) {
                let api = api.clone();
                cx.background_executor()
                    .spawn(smol::unblock(move || api.discover_features()))
                    .detach();
            }
            self.sessions.insert(server.id, api);
        }
        if restored_active {
            self.verify_active_server(cx);
        } else {
            self.reload(false, cx);
        }
    }

    pub fn verify_active_server(&mut self, cx: &mut Context<Self>) {
        self.server_check.cancel();
        let Some(api) = self.api() else {
            self.reload(false, cx);
            return;
        };
        let server = api.server.id;
        let generation = self.server_check.generation;
        self.invalidate_library_data();
        self.server_check.busy = Some(server);
        self.message.clear();
        let task = cx.background_executor().spawn(async move {
            let mut started = std::time::Instant::now();
            let mut result = smol::unblock({
                let api = api.clone();
                move || api.validate_session()
            })
            .await;
            if result
                .as_ref()
                .is_err_and(feishin_core::api::is_network_error)
            {
                log::warn!("Server account check failed; retrying once: {server}");
                smol::Timer::after(std::time::Duration::from_millis(500)).await;
                started = std::time::Instant::now();
                result = smol::unblock({
                    let api = api.clone();
                    move || api.validate_session()
                })
                .await;
            }
            if result.is_ok() {
                smol::unblock(move || api.discover_features()).await;
                smol::Timer::after(
                    std::time::Duration::from_secs(1).saturating_sub(started.elapsed()),
                )
                .await;
            }
            result
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.server_check.generation != generation
                    || this.state.active_server != Some(server)
                {
                    return;
                }
                this.server_check.busy = None;
                match result {
                    Ok(()) => this.reload(false, cx),
                    Err(error) => {
                        log::warn!("Server account check failed for {server}: {error}");
                        this.loading = false;
                        if feishin_core::api::is_network_error(&error) {
                            // Replace this visit, preserving account credentials and queue identities.
                            this.route = crate::app::Route::NoNetwork;
                            this.message.clear();
                        } else {
                            this.route = crate::app::Route::Servers;
                            this.message = error.to_string();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn retry_server_connection(&mut self, cx: &mut Context<Self>) {
        if self.server_check.busy.is_some() {
            return;
        }
        self.verify_active_server(cx);
        self.navigate(crate::app::Route::Home, cx);
    }

    pub fn server_connection_view(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_component::{Icon, Sizable, button::ButtonVariants, spinner::Spinner};
        let checking = self.server_check.busy.is_some();
        let root = f32::from(cx.theme().font_size);
        if checking {
            return div()
                .id("server-connection-check")
                .role(Role::Status)
                .aria_label(self.tr("Connecting..."))
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Spinner::new()
                        .icon(Icon::empty().path("native-icons/spinner.svg"))
                        .with_size(px(root * 1.125))
                        .animation(std::time::Duration::from_secs(1), |value| value),
                )
                .into_any_element();
        }
        div()
            .size_full()
            .v_flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .v_flex()
                    .items_center()
                    .gap(px(root * 2.))
                    .max_w(relative(0.5))
                    .text_center()
                    .child(
                        Icon::empty()
                            .path("native-icons/wifi-off.svg")
                            .size(px(root * 4.)),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap(px(root))
                            .child(
                                div()
                                    .text_size(px(root * 1.25))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.tr("error.noNetwork")),
                            )
                            .child(
                                div()
                                    .text_size(px(root * 0.875))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.tr("error.noNetworkDescription")),
                            ),
                    )
                    .child(
                        crate::styles::button("retry-server-connection", cx)
                            .debug_selector(|| "retry-server-connection".into())
                            .primary()
                            .icon(Icon::empty().path("native-icons/refresh.svg"))
                            .label(self.tr("common.retry"))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.retry_server_connection(cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub fn discover_servers(&mut self, cx: &mut Context<Self>) {
        self.server_discovery = Discovery::default();
        self.server_discovery.started = true;
        self.server_discovery.busy = true;
        let (sender, receiver) = std::sync::mpsc::sync_channel(65);
        self.server_discovery.receiver = Some(receiver);
        let cancelled = self.server_discovery.cancelled.clone();
        cx.background_executor()
            .spawn(async move {
                let result = feishin_core::server_discovery::jellyfin(&cancelled, |server| {
                    sender.try_send(DiscoveryEvent::Found(server)).is_ok()
                });
                let _ = sender.try_send(DiscoveryEvent::Finished(
                    result.map_err(|error| error.to_string()),
                ));
            })
            .detach();
        cx.notify();
    }
    pub fn poll_server_discovery(&mut self, cx: &mut Context<Self>) {
        if self.route != crate::app::Route::Servers {
            if self.server_discovery.busy {
                self.server_discovery
                    .cancelled
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                self.server_discovery.receiver = None;
                self.server_discovery.busy = false;
            }
            self.server_discovery.started = false;
            return;
        }
        if !cfg!(test) && !self.server_discovery.started && self.editing_server.is_none() {
            self.discover_servers(cx);
        }
        let events: Vec<_> = self
            .server_discovery
            .receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                DiscoveryEvent::Found(server) => {
                    if !self
                        .server_discovery
                        .servers
                        .iter()
                        .any(|known| known.url == server.url)
                    {
                        self.server_discovery.servers.push(server);
                    }
                }
                DiscoveryEvent::Finished(result) => {
                    self.server_discovery.busy = false;
                    self.server_discovery.receiver = None;
                    if let Err(error) = result {
                        log::warn!("Server discovery failed: {error}");
                        self.server_discovery.error = Some(error);
                    }
                }
            }
            cx.notify();
        }
    }
    pub fn select_discovered_server(
        &mut self,
        server: &feishin_core::server_discovery::Server,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_server(None, window, cx);
        self.server_kind = ServerType::Jellyfin;
        self.server_name.update(cx, |input, cx| {
            input.set_value(server.name.clone(), window, cx)
        });
        self.server_url.update(cx, |input, cx| {
            input.set_value(server.url.clone(), window, cx)
        });
        cx.notify();
    }
    pub fn server_discovery_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_2()
            .child(
                crate::styles::button("discover-servers", cx)
                    .label(self.tr(if self.server_discovery.busy {
                        "Discovering servers..."
                    } else {
                        "Discover servers"
                    }))
                    .disabled(self.server_discovery.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.discover_servers(cx))),
            )
            .children(
                self.server_discovery
                    .error
                    .iter()
                    .map(|error| div().text_sm().child(error.clone())),
            )
            .children(
                self.server_discovery
                    .servers
                    .iter()
                    .enumerate()
                    .map(|(index, server)| {
                        let server = server.clone();
                        crate::styles::button(("discovered-server", index), cx)
                            .debug_selector(move || format!("discovered-server-{index}"))
                            .label(format!("{} - Jellyfin ({})", server.name, server.url))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_discovered_server(&server, window, cx)
                            }))
                    }),
            )
            .into_any_element()
    }
    pub(crate) fn sync_tls_settings(&mut self, cx: &mut Context<Self>) {
        let ignore_ssl = self.state.settings.ignore_ssl;
        self.server_options.ignore_ssl = ignore_ssl;
        for server in &mut self.state.servers {
            server.options.ignore_ssl = ignore_ssl;
        }
        let clients = self
            .sessions
            .values()
            .filter(|api| api.server.options.ignore_ssl != ignore_ssl)
            .cloned()
            .collect::<Vec<_>>();
        if !clients.is_empty() {
            self.reset_waveform();
            cx.notify();
        }
        for api in clients {
            let mut server = api.server.clone();
            server.options.ignore_ssl = ignore_ssl;
            match api.reconfigure(server) {
                Ok(api) => {
                    self.sessions.insert(api.server.id, api);
                }
                Err(error) => {
                    log::warn!("Could not update server TLS policy: {error}");
                    self.message = format!("Could not update server TLS policy: {error}");
                }
            }
        }
    }
    pub(crate) fn server_tls_control(&self, cx: &mut Context<Self>) -> AnyElement {
        Checkbox::new("ignore-ssl")
            .debug_selector(|| "ignore-ssl".into())
            .label(self.tr("form.addServer.ignoreSsl"))
            .when(crate::appearance::rtl(cx), |checkbox| {
                checkbox.flex_row_reverse()
            })
            .checked(self.state.settings.ignore_ssl)
            .on_click(cx.listener(|this, value: &bool, _, cx| {
                this.cancel_quick_connect();
                this.state.settings.ignore_ssl = *value;
                this.sync_tls_settings(cx);
                this.save();
                cx.notify();
            }))
            .into_any_element()
    }
    pub fn expire_server_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let expired = self
            .sessions
            .iter()
            .filter(|(_, api)| api.is_signed_out())
            .map(|(id, api)| (*id, api.server.name.clone()))
            .collect::<Vec<_>>();
        for (id, name) in expired {
            log::warn!("Server session expired: {id}");
            self.sign_out_server(id, window, cx);
            if self.message == self.tr("Signed out") {
                self.message = format!("{name}: {}", self.tr("error.sessionExpiredError"));
            }
        }
    }
    pub fn sign_out_server(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        if self.signing_out.contains(&id) {
            return;
        }
        let Some(server) = self
            .state
            .servers
            .iter()
            .find(|server| server.id == id)
            .cloned()
        else {
            return;
        };
        if self.server_check.busy == Some(id) {
            self.server_check.cancel();
        }
        self.cancel_quick_connect();
        if self
            .current_metadata()
            .is_some_and(|item| item.server_id == id)
        {
            self.stop_transport(cx);
        }
        let api = self.sessions.remove(&id);
        self.lyric_selections.retain(|(server, _), _| *server != id);
        if let Some(api) = &api {
            api.sign_out();
        }
        self.signing_out.insert(id);
        self.state.signed_out_servers.insert(id);
        if let Some(remote) = &self.remote {
            remote.forget_server(id);
        }
        if self.state.active_server == Some(id) {
            self.close_command_palette(window, cx);
            self.state.active_server = None;
            self.reset_library_scan();
            self.invalidate_library_data();
            self.lyric_generation += 1;
            self.lyric_busy = false;
            self.lyric_record = None;
            self.lyrics.clear();
            self.reset_lyric_translation(cx);
            self.annotation_generation += 1;
            self.lyric_annotations.clear();
            self.navigate(crate::app::Route::Servers, cx);
            self.history.clear();
            self.forward_history.clear();
            self.edit_server(Some(id), window, cx);
        }
        self.message = self.tr("Signed out");
        self.save();
        let task = cx.background_executor().spawn(async move {
            api.map_or_else(
                || feishin_core::storage::delete_session(&server),
                |api| api.forget_saved_session(),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.signing_out.remove(&id);
                if let Err(error) = result {
                    this.message =
                        format!("Signed out. Could not remove saved credentials: {error}");
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn server_from_form(&self, cx: &App) -> Server {
        let url = self.server_url.read(cx).value().trim().to_owned();
        let username = self.username.read(cx).value().trim().to_owned();
        let id = self.editing_server.unwrap_or_else(|| {
            self.state
                .servers
                .iter()
                .find(|server| {
                    server.kind == self.server_kind
                        && server.username == username
                        && feishin_core::api::normalize_url(&server.url).ok()
                            == feishin_core::api::normalize_url(&url).ok()
                })
                .map_or_else(Uuid::new_v4, |server| server.id)
        });
        Server {
            id,
            name: self.server_name.read(cx).value().to_string(),
            url,
            username,
            kind: self.server_kind,
            legacy_auth: self.legacy_auth,
            options: self.form_server_options(cx),
        }
    }

    pub fn form_server_options(&self, cx: &App) -> ServerOptions {
        let mut options = self.server_options.clone();
        options.ignore_ssl = self.state.settings.ignore_ssl;
        options.remote_url = self.server_remote_url.read(cx).value().trim().into();
        options.music_folder_ids = self
            .server_libraries
            .read(cx)
            .value()
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .collect();
        let mut seen = std::collections::HashSet::new();
        options
            .music_folder_ids
            .retain(|id| seen.insert(id.clone()));
        options
    }
    pub fn edit_server(&mut self, id: Option<Uuid>, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_quick_connect();
        self.quick_connect.selected = false;
        self.remember = id.is_none_or(|id| !self.state.signed_out_servers.contains(&id));
        self.editing_server = id;
        self.server_library_choices.clear();
        let server = id
            .and_then(|id| self.state.servers.iter().find(|server| server.id == id))
            .cloned();
        self.server_options = server.as_ref().map_or_else(
            || ServerOptions {
                remote_url: self.server_defaults.remote_url.clone(),
                ..Default::default()
            },
            |server| server.options.clone(),
        );
        self.server_kind = server
            .as_ref()
            .map_or(self.server_defaults.kind, |server| server.kind);
        self.legacy_auth = server
            .as_ref()
            .map_or(self.server_defaults.legacy_auth, |server| {
                server.legacy_auth
            });
        for (input, value) in [
            (
                &self.server_name,
                server.as_ref().map_or_else(
                    || self.server_defaults.name.clone(),
                    |server| server.name.clone(),
                ),
            ),
            (
                &self.server_url,
                server.as_ref().map_or_else(
                    || self.server_defaults.url.clone(),
                    |server| server.url.clone(),
                ),
            ),
            (
                &self.username,
                server
                    .as_ref()
                    .map_or(String::new(), |server| server.username.clone()),
            ),
            (&self.password, String::new()),
            (
                &self.server_remote_url,
                self.server_options.remote_url.clone(),
            ),
            (
                &self.server_libraries,
                self.server_options.music_folder_ids.join(","),
            ),
        ] {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        if let Some(api) = id.and_then(|id| self.sessions.get(&id)).cloned() {
            let task = cx.background_executor().spawn(async move {
                api.list(&ListQuery {
                    kind: ItemKind::Folder,
                    limit: 500,
                    ..Default::default()
                })
            });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                this.update(cx, |this, cx| {
                    if this.editing_server != id {
                        return;
                    }
                    match result {
                        Ok(page) => this.server_library_choices = page.items,
                        Err(error) => log::debug!("Server libraries unavailable: {error}"),
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }
    pub fn save_server_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.editing_server else {
            return;
        };
        let Some(old) = self
            .state
            .servers
            .iter()
            .find(|server| server.id == id)
            .cloned()
        else {
            return;
        };
        if self.server_kind == ServerType::Navidrome
            && self.server_options.save_password
            && !old.options.save_password
            && self
                .sessions
                .get(&id)
                .is_none_or(|api| api.session().refresh_password.is_none())
        {
            if self.password.read(cx).value().is_empty() {
                self.message = "Enter your password and sign in to save it".into();
                cx.notify();
            } else {
                self.login(window, cx);
            }
            return;
        }
        if old.username != self.username.read(cx).value().trim()
            || old.kind != self.server_kind
            || old.url.trim_end_matches('/')
                != self
                    .server_url
                    .read(cx)
                    .value()
                    .trim()
                    .trim_end_matches('/')
            || old.legacy_auth != self.legacy_auth
        {
            if self.server_kind == ServerType::Jellyfin && self.quick_connect.selected {
                self.start_quick_connect(window, cx);
            } else if self.password.read(cx).value().is_empty() {
                self.message =
                    "Enter your password and sign in to change the server address or account"
                        .into();
                cx.notify();
            } else {
                self.login(window, cx);
            }
            return;
        }
        let server = Server {
            id,
            name: self.server_name.read(cx).value().trim().into(),
            options: self.form_server_options(cx),
            ..old
        };
        if server.name.is_empty() {
            self.message = "Enter a server name".into();
            cx.notify();
            return;
        }
        let configured = self.sessions.get(&id).map_or_else(
            || Api::new(server.clone(), Default::default()),
            |api| api.reconfigure(server.clone()),
        );
        match configured {
            Ok(api) => {
                if self.state.active_server == Some(id) {
                    self.reset_library_scan();
                }
                let server = api.server.clone();
                if self.sessions.contains_key(&id) {
                    self.sessions.insert(id, api);
                }
                *self
                    .state
                    .servers
                    .iter_mut()
                    .find(|server| server.id == id)
                    .unwrap() = server;
                self.reset_playlists();
                self.save();
                self.message = "Server settings saved".into();
            }
            Err(error) => self.message = error.to_string(),
        }
        cx.notify();
    }
    pub fn server_options_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_3()
            .when(self.server_kind == ServerType::Navidrome, |div| {
                div.child(
                    Checkbox::new("server-save-password")
                        .debug_selector(|| "server-save-password".into())
                        .label(self.tr("form.addServer.input_savePassword"))
                        .when(crate::appearance::rtl(cx), |checkbox| {
                            checkbox.flex_row_reverse()
                        })
                        .checked(self.server_options.save_password)
                        .on_click(cx.listener(|this, value: &bool, _, cx| {
                            this.cancel_quick_connect();
                            this.server_options.save_password = *value;
                            cx.notify();
                        })),
                )
            })
            .child(self.server_input(
                "form.addServer.input_remoteUrl",
                crate::styles::input(&self.server_remote_url, cx),
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.tr("form.addServer.input_remoteUrlDescription")),
            )
            .when(!self.server_remote_url.read(cx).value().is_empty(), |div| {
                div.child(
                    Checkbox::new("server-prefer-remote")
                        .debug_selector(|| "server-prefer-remote".into())
                        .label(self.tr("form.addServer.input_preferRemoteUrl"))
                        .when(crate::appearance::rtl(cx), |checkbox| {
                            checkbox.flex_row_reverse()
                        })
                        .checked(self.server_options.prefer_remote_url)
                        .on_click(cx.listener(|this, value: &bool, _, cx| {
                            this.cancel_quick_connect();
                            this.server_options.prefer_remote_url = *value;
                            cx.notify();
                        })),
                )
            })
            .child(crate::styles::input(&self.server_libraries, cx))
            .child(
                crate::styles::button("server-select-libraries", cx)
                    .label(crate::appearance::text("Choose libraries", cx))
                    .disabled(self.server_library_choices.is_empty())
                    .dropdown_menu({
                        let view = cx.weak_entity();
                        let choices = self.server_library_choices.clone();
                        let selected = self.form_server_options(cx).music_folder_ids;
                        move |mut menu, _, cx| {
                            let clear = view.clone();
                            menu = menu.item(
                                PopupMenuItem::new(crate::appearance::text("All libraries", cx))
                                    .on_click(move |_, window, cx| {
                                        clear
                                            .update(cx, |this, cx| {
                                                this.server_libraries.update(cx, |input, cx| {
                                                    input.set_value("", window, cx)
                                                })
                                            })
                                            .ok();
                                    }),
                            );
                            for item in &choices {
                                let view = view.clone();
                                let id = item.id.clone();
                                let label = format!(
                                    "{}{}",
                                    if selected.contains(&id) { "✓ " } else { "" },
                                    item.name
                                );
                                menu = menu.item(PopupMenuItem::new(label).on_click(
                                    move |_, window, cx| {
                                        view.update(cx, |this, cx| {
                                            let mut libraries =
                                                this.form_server_options(cx).music_folder_ids;
                                            if libraries.contains(&id) {
                                                libraries.retain(|value| value != &id);
                                            } else {
                                                libraries.push(id.clone());
                                            }
                                            this.server_libraries.update(cx, |input, cx| {
                                                input.set_value(libraries.join(","), window, cx)
                                            });
                                            cx.notify();
                                        })
                                        .ok();
                                    },
                                ));
                            }
                            menu
                        }
                    }),
            )
            .when(self.server_kind == ServerType::Jellyfin, |div| {
                div.child(
                    Checkbox::new("server-instant-mix")
                        .debug_selector(|| "server-instant-mix".into())
                        .label(self.tr("form.addServer.input_preferInstantMix"))
                        .when(crate::appearance::rtl(cx), |checkbox| {
                            checkbox.flex_row_reverse()
                        })
                        .child(
                            gpui::div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.tr("form.addServer.input_preferInstantMixDescription")),
                        )
                        .checked(self.server_options.prefer_instant_mix)
                        .on_click(cx.listener(|this, value: &bool, _, cx| {
                            this.cancel_quick_connect();
                            this.server_options.prefer_instant_mix = *value;
                            cx.notify();
                        })),
                )
                .child(
                    Checkbox::new("server-audiobooks")
                        .debug_selector(|| "server-audiobooks".into())
                        .label(self.tr("form.addServer.input_enableAudiobooks"))
                        .when(crate::appearance::rtl(cx), |checkbox| {
                            checkbox.flex_row_reverse()
                        })
                        .child(
                            gpui::div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.tr("form.addServer.input_enableAudiobooksDescription")),
                        )
                        .checked(self.server_options.enable_audiobooks)
                        .on_click(cx.listener(|this, value: &bool, _, cx| {
                            this.cancel_quick_connect();
                            this.server_options.enable_audiobooks = *value;
                            cx.notify();
                        })),
                )
            })
            .when(self.editing_server.is_some(), |div| {
                div.child(
                    crate::styles::button("save-server-options", cx)
                        .debug_selector(|| "save-server-options".into())
                        .label(crate::appearance::text("Save server settings", cx))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.save_server_options(window, cx)),
                        ),
                )
            })
            .into_any_element()
    }
}

pub(crate) fn prepare_login(
    api: Api,
    remember: bool,
    cancelled: Option<&std::sync::atomic::AtomicBool>,
) -> anyhow::Result<(Api, Option<String>)> {
    let check_cancelled = || -> anyhow::Result<()> {
        anyhow::ensure!(
            !cancelled.is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)),
            "Quick Connect cancelled"
        );
        Ok(())
    };
    check_cancelled()?;
    api.discover_features();
    check_cancelled()?;
    let warning = if remember {
        api.remember_session()
            .err()
            .map(|error| format!("Signed in for this session. {error}"))
    } else {
        feishin_core::storage::delete_session(&api.server)
            .err()
            .map(|error| format!("Signed in. Could not remove saved credentials: {error}"))
    };
    Ok((api, warning))
}

#[cfg(test)]
mod tests {
    use super::{Discovery, DiscoveryEvent};
    use crate::app::{Feishin, Route};
    use feishin_core::{
        api::Api,
        model::{Item, ItemKind, Server, ServerType, Session},
        queue::Enqueue,
        storage,
    };
    use gpui::{
        AppContext, Context, Entity, Modifiers, Render, TestAppContext, Window, prelude::*,
    };
    use gpui_component::Root;
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    struct Probe(Entity<Feishin>);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0.update(cx, |view, cx| view.sidebar_view(cx))
        }
    }

    #[gpui::test]
    fn server_form_labels_and_option_clicks_match_original_forms(cx: &mut TestAppContext) {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/server-auth-reference.json")).unwrap();
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-server-labels-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = storage::State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            let view = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(gpui::size(gpui::px(1200.), gpui::px(1800.)));
        cx.run_until_parked();
        for language in ["en", "fr", "ar"] {
            for case in reference["forms"].as_array().unwrap() {
                cx.update(|window, cx| {
                    view.update(cx, |view, cx| {
                        view.route = Route::Servers;
                        view.state.settings.language = language.into();
                        view.server_kind = match case["kind"].as_str().unwrap() {
                            "jellyfin" => ServerType::Jellyfin,
                            "subsonic" => ServerType::Subsonic,
                            _ => ServerType::Navidrome,
                        };
                        view.server_options = Default::default();
                        view.legacy_auth = false;
                        view.quick_connect.selected = case["quick"].as_bool().unwrap();
                        view.server_remote_url.update(cx, |input, cx| {
                            input.set_value(
                                if case["remote"].as_bool().unwrap() {
                                    "https://public.example"
                                } else {
                                    ""
                                },
                                window,
                                cx,
                            )
                        });
                        view.username
                            .update(cx, |input, cx| input.set_value("listener", window, cx));
                        view.password.update(cx, |input, cx| {
                            input.set_value("fixture-password", window, cx)
                        });
                        view.apply_theme(window, cx);
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                let tree =
                    cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
                let remote_label =
                    feishin_core::i18n::text(language, "form.addServer.input_preferRemoteUrl");
                assert_eq!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.role() == gpui::Role::CheckBox
                            && node.label() == Some(remote_label.as_str())),
                    case["remote"].as_bool().unwrap()
                );
                for field in case["fields"].as_array().unwrap() {
                    let name = field["field"].as_str().unwrap();
                    let label =
                        feishin_core::i18n::text(language, field["label"].as_str().unwrap());
                    assert!(
                        tree.nodes
                            .iter()
                            .any(|(_, node)| node.label() == Some(label.as_str())),
                        "Missing {name} in {language}: {case}"
                    );
                    if let Some(description) = field["description"].as_str() {
                        let description = feishin_core::i18n::text(language, description);
                        assert!(
                            tree.nodes
                                .iter()
                                .any(|(_, node)| node.value() == Some(description.as_str())
                                    || node.label() == Some(description.as_str())),
                            "Missing description for {name} in {language}"
                        );
                    }
                    let selector = match name {
                        "savePassword" => "server-save-password",
                        "preferRemoteUrl" => "server-prefer-remote",
                        "preferInstantMix" => "server-instant-mix",
                        "enableAudiobooks" => "server-audiobooks",
                        "legacyAuth" => "server-legacy",
                        _ => continue,
                    };
                    for checked in [true, false] {
                        let cancelled =
                            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        cx.update(|_, cx| {
                            view.update(cx, |view, cx| {
                                view.quick_connect.cancelled = cancelled.clone();
                                view.quick_connect.busy = true;
                                cx.notify();
                            })
                        });
                        cx.run_until_parked();
                        if checked {
                            let point = cx.debug_bounds(selector).unwrap().center();
                            cx.simulate_click(point, Modifiers::none());
                        } else {
                            cx.update(|window, cx| {
                                use gpui::accesskit::{Action, ActionRequest, TreeId};
                                let node = window
                                    .accessibility_tree_for_test()
                                    .unwrap()
                                    .nodes
                                    .iter()
                                    .find(|(_, node)| {
                                        node.role() == gpui::Role::CheckBox
                                            && node.label() == Some(label.as_str())
                                    })
                                    .unwrap()
                                    .0;
                                window.dispatch_accessibility_action_for_test(
                                    ActionRequest {
                                        action: Action::Focus,
                                        target_tree: TreeId::ROOT,
                                        target_node: node,
                                        data: None,
                                    },
                                    cx,
                                );
                            });
                            cx.run_until_parked();
                            cx.simulate_keystrokes("space");
                            cx.simulate_event(gpui::KeyUpEvent {
                                keystroke: gpui::Keystroke::parse("space").unwrap(),
                            });
                        }
                        cx.run_until_parked();
                        assert!(
                            cancelled.load(std::sync::atomic::Ordering::Relaxed),
                            "{name}, {language}, checked={checked}, {case}"
                        );
                        view.read_with(cx, |view, cx| {
                            let server = view.server_from_form(cx);
                            assert_eq!(
                                match name {
                                    "savePassword" => server.options.save_password,
                                    "preferRemoteUrl" => server.options.prefer_remote_url,
                                    "preferInstantMix" => server.options.prefer_instant_mix,
                                    "enableAudiobooks" => server.options.enable_audiobooks,
                                    _ => server.legacy_auth,
                                },
                                checked
                            );
                            assert_eq!(view.username.read(cx).value(), "listener");
                            assert_eq!(view.password.read(cx).value(), "fixture-password");
                            assert!(!view.playing);
                            assert!(!view.quick_connect.busy);
                        });
                        let tree = cx.update(|window, _| {
                            window.accessibility_tree_for_test().unwrap().clone()
                        });
                        assert!(tree.nodes.iter().any(|(_, node)| node.role()
                            == gpui::Role::CheckBox
                            && node.label() == Some(label.as_str())
                            && node.toggled()
                                == Some(if checked {
                                    gpui::Toggled::True
                                } else {
                                    gpui::Toggled::False
                                })));
                    }
                }
            }
        }
        let id = uuid::Uuid::new_v4();
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                let server = Server {
                    id,
                    name: "Token-only session".into(),
                    url: "https://fixture.invalid".into(),
                    username: "listener".into(),
                    kind: ServerType::Navidrome,
                    legacy_auth: false,
                    options: Default::default(),
                };
                view.state.settings.language = "en".into();
                view.state.servers.push(server.clone());
                view.edit_server(Some(id), window, cx);
                view.sessions.insert(
                    id,
                    Api::new(
                        server,
                        Session {
                            token: "saved-token".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                );
                view.server_options.save_password = true;
                view.apply_theme(window, cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
        let save = cx.debug_bounds("save-server-options").unwrap().center();
        cx.simulate_click(save, Modifiers::none());
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(view.message.contains("Enter your password"));
            assert!(
                !view
                    .state
                    .servers
                    .iter()
                    .find(|server| server.id == id)
                    .unwrap()
                    .options
                    .save_password
            );
            assert_eq!(view.sessions[&id].session().token, "saved-token");
        });
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                let server = view.sessions[&id].server.clone();
                view.sessions.insert(
                    id,
                    Api::new(
                        server,
                        Session {
                            token: "saved-token".into(),
                            refresh_password: Some("in-memory-password".into()),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                );
                cx.notify();
            });
        });
        cx.run_until_parked();
        let save = cx.debug_bounds("save-server-options").unwrap().center();
        cx.simulate_click(save, Modifiers::none());
        cx.run_until_parked();
        assert!(
            storage::load(&directory.join("state.json"))
                .unwrap()
                .servers
                .iter()
                .find(|server| server.id == id)
                .unwrap()
                .options
                .save_password
        );
        view.read_with(cx, |view, _| {
            assert_eq!(view.sessions[&id].session().token, "saved-token")
        });
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn saved_account_network_retry_permissions_and_switches_preserve_queue(
        cx: &mut TestAppContext,
    ) {
        use std::{
            io::{BufRead, BufReader, Write},
            net::TcpListener,
            sync::{
                Arc, Mutex,
                atomic::{AtomicBool, AtomicU8, Ordering},
            },
        };
        struct Fixture {
            server: Server,
            mode: Arc<AtomicU8>,
            requests: Arc<Mutex<Vec<String>>>,
            stop: Arc<AtomicBool>,
            worker: Option<std::thread::JoinHandle<()>>,
        }
        impl Fixture {
            fn new() -> Self {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                listener.set_nonblocking(true).unwrap();
                let server = Server {
                    id: uuid::Uuid::new_v4(),
                    name: "Fixture".into(),
                    url: format!("http://{}", listener.local_addr().unwrap()),
                    username: "listener".into(),
                    kind: ServerType::Jellyfin,
                    legacy_auth: false,
                    options: Default::default(),
                };
                let mode = Arc::new(AtomicU8::new(0));
                let requests = Arc::new(Mutex::new(Vec::new()));
                let stop = Arc::new(AtomicBool::new(false));
                let (mode_thread, requests_thread, stop_thread) =
                    (mode.clone(), requests.clone(), stop.clone());
                let worker = std::thread::spawn(move || {
                    while !stop_thread.load(Ordering::SeqCst) {
                        let Ok((mut stream, _)) = listener.accept() else {
                            std::thread::sleep(Duration::from_millis(5));
                            continue;
                        };
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        let mut reader = BufReader::new(stream.try_clone().unwrap());
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_err() {
                            continue;
                        }
                        let mut headers = String::new();
                        loop {
                            let mut header = String::new();
                            if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
                                break;
                            }
                            headers.push_str(&header);
                        }
                        if line.starts_with("GET /Users/account/Items?") {
                            assert!(line.contains("IncludeItemTypes=Playlist"), "{line}");
                            let body = r#"{"Items":[],"TotalRecordCount":0}"#;
                            let _ = write!(
                                stream,
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            continue;
                        }
                        requests_thread.lock().unwrap().push(line.trim().to_owned());
                        let deadline = Instant::now() + Duration::from_secs(10);
                        while mode_thread.load(Ordering::SeqCst) == 3
                            && !stop_thread.load(Ordering::SeqCst)
                            && Instant::now() < deadline
                        {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        let mode = mode_thread.load(Ordering::SeqCst);
                        if mode == 0 || mode == 6 {
                            if mode == 6 {
                                mode_thread.store(1, Ordering::SeqCst);
                            }
                            let _ = write!(
                                stream,
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 999\r\nConnection: close\r\n\r\n{{"
                            );
                            continue;
                        }
                        let login = line.starts_with("POST /Users/AuthenticateByName ");
                        let body = if login {
                            serde_json::json!({"AccessToken":"renewed-token","User":{"Id":"account","Policy":{"IsAdministrator":true}}})
                        } else {
                            serde_json::json!({"Id":"account","Policy":{"IsAdministrator":mode==1 || mode==7}})
                        }.to_string();
                        let status = if mode == 5 {
                            500
                        } else if mode == 7
                            && !login
                            && !headers.contains("Token=\"renewed-token\"")
                        {
                            401
                        } else {
                            200
                        };
                        let _ = write!(
                            stream,
                            "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                    }
                });
                Self {
                    server,
                    mode,
                    requests,
                    stop,
                    worker: Some(worker),
                }
            }
            fn api(&self) -> Api {
                Api::new(
                    self.server.clone(),
                    Session {
                        token: "private-token".into(),
                        user_id: "account".into(),
                        ..Default::default()
                    },
                )
                .unwrap()
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                self.stop.store(true, Ordering::SeqCst);
                self.worker.take().unwrap().join().unwrap();
            }
        }
        fn wait(
            cx: &mut gpui::VisualTestContext,
            mut ready: impl FnMut(&mut gpui::VisualTestContext) -> bool,
        ) {
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                cx.run_until_parked();
                if ready(cx) {
                    break;
                }
                assert!(Instant::now() < deadline, "Account check did not finish");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/server-auth-reference.json")).unwrap();
        let fixture = Fixture::new();
        let second = Fixture::new();
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        second.mode.store(1, Ordering::SeqCst);
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-account-check-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = storage::State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.language = "fr".into();
            let view = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.state.servers = vec![fixture.server.clone(), second.server.clone()];
                view.state.servers[0].url.push('/');
                view.state.servers[0].options.ignore_ssl = true;
                view.state.servers[0].options.music_folder_ids = vec!["selected".into()];
                view.sessions.insert(second.server.id, second.api());
                let mut signed_out = fixture.server.clone();
                signed_out.id = uuid::Uuid::new_v4();
                view.state.servers.push(signed_out.clone());
                view.state.signed_out_servers.insert(signed_out.id);
                let mut removed = fixture.server.clone();
                removed.id = uuid::Uuid::new_v4();
                let mut replaced = fixture.server.clone();
                replaced.url = second.server.url.clone();
                let song = Item {
                    id: "song".into(),
                    server_id: fixture.server.id,
                    ..Default::default()
                };
                view.state.queue.add(vec![song.clone(), song], Enqueue::Now);
                view.state.queue.current = view.state.queue.entries.last().map(|entry| entry.key);
                view.state.active_server = Some(fixture.server.id);
                view.route = Route::Home;
                let stale = Session {
                    token: "stale-token".into(),
                    ..fixture.api().session()
                };
                view.restore_server_sessions(
                    vec![
                        (signed_out.clone(), stale.clone()),
                        (removed, stale.clone()),
                        (replaced, stale.clone()),
                        (second.server.clone(), stale),
                        (fixture.server.clone(), fixture.api().session()),
                    ],
                    cx,
                );
                assert_eq!(view.sessions.len(), 2);
                assert_eq!(
                    view.sessions[&second.server.id].session().token,
                    "private-token"
                );
                let restored = &view.sessions[&fixture.server.id];
                assert_eq!(restored.session().token, "private-token");
                assert!(restored.server.options.ignore_ssl);
                assert_eq!(restored.server.options.music_folder_ids, ["selected"]);
                view.state
                    .servers
                    .retain(|server| server.id != signed_out.id);
                view.state.signed_out_servers.remove(&signed_out.id);
                // A restored expanded player must not cover the account check or Retry button.
                view.full_player = true;
                assert_eq!(view.server_check.busy, Some(fixture.server.id));
            })
        });
        let keys = view.read_with(cx, |view, _| {
            view.state
                .queue
                .entries
                .iter()
                .map(|entry| entry.key)
                .collect::<Vec<_>>()
        });
        wait(cx, |cx| {
            view.read_with(cx, |view, _| view.route == Route::NoNetwork)
        });
        let unavailable = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "network-unavailable")
            .unwrap();
        let requests = fixture.requests.lock().unwrap().clone();
        assert_eq!(
            requests.len(),
            unavailable["attempts"].as_u64().unwrap() as usize,
            "{requests:?}"
        );
        for request in fixture.requests.lock().unwrap().iter() {
            assert_eq!(request, "GET /Users/account HTTP/1.1");
        }
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        let notice = feishin_core::i18n::text("fr", "error.noNetworkDescription");
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.value() == Some(notice.as_str())
                    || node.label() == Some(notice.as_str()))
        );
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.sessions[&fixture.server.id].session().token,
                "private-token"
            );
            assert!(!view.state.signed_out_servers.contains(&fixture.server.id));
        });
        fixture.mode.store(1, Ordering::SeqCst);
        let point = cx.debug_bounds("retry-server-connection").unwrap().center();
        cx.simulate_click(point, Modifiers::none());
        wait(cx, |cx| {
            view.read_with(cx, |view, _| view.server_check.busy.is_none())
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Home);
            assert!(view.sessions[&fixture.server.id].session().is_admin);
        });
        let before = fixture.requests.lock().unwrap().len();
        cx.update(|_, cx| view.update(cx, |view, cx| view.back(cx)));
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::NoNetwork);
            assert!(view.server_check.busy.is_none());
        });
        assert_eq!(fixture.requests.lock().unwrap().len(), before);
        cx.update(|_, cx| view.update(cx, |view, cx| view.forward(cx)));
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |view, _| view.route.clone()),
            Route::Home
        );
        fixture.mode.store(7, Ordering::SeqCst);
        let before = fixture.requests.lock().unwrap().len();
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                let mut session = view.sessions[&fixture.server.id].session();
                session.refresh_password = Some("fixture-password".into());
                view.sessions.insert(
                    fixture.server.id,
                    Api::new(fixture.server.clone(), session).unwrap(),
                );
                view.choose_server(fixture.server.id, cx);
            })
        });
        wait(cx, |cx| {
            view.read_with(cx, |view, _| view.server_check.busy.is_none())
        });
        let renewed = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "expired-saved-password")
            .unwrap();
        assert_eq!(
            renewed["states"].as_array().unwrap().last().unwrap(),
            "valid"
        );
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Home);
            assert_eq!(
                view.sessions[&fixture.server.id].session().token,
                "renewed-token"
            );
            assert!(view.sessions[&fixture.server.id].session().is_admin);
        });
        let requests = fixture.requests.lock().unwrap().clone();
        assert_eq!(requests.len() - before, 3);
        assert_eq!(
            requests[before + 1],
            "POST /Users/AuthenticateByName HTTP/1.1"
        );
        fixture.mode.store(4, Ordering::SeqCst);
        cx.update(|_, cx| view.update(cx, |view, cx| view.choose_server(fixture.server.id, cx)));
        wait(cx, |cx| {
            view.read_with(cx, |view, _| view.server_check.busy.is_none())
        });
        assert!(!view.read_with(cx, |view, _| {
            view.sessions[&fixture.server.id].session().is_admin
        }));
        fixture.mode.store(6, Ordering::SeqCst);
        let before = fixture.requests.lock().unwrap().len();
        cx.update(|_, cx| view.update(cx, |view, cx| view.choose_server(fixture.server.id, cx)));
        wait(cx, |cx| {
            view.read_with(cx, |view, _| view.server_check.busy.is_none())
        });
        let recovered = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "network-recovered")
            .unwrap();
        assert_eq!(
            fixture.requests.lock().unwrap().len() - before,
            recovered["attempts"].as_u64().unwrap() as usize
        );
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Home);
            assert!(view.sessions[&fixture.server.id].session().is_admin);
        });
        fixture.mode.store(5, Ordering::SeqCst);
        let before = fixture.requests.lock().unwrap().len();
        cx.update(|_, cx| view.update(cx, |view, cx| view.choose_server(fixture.server.id, cx)));
        wait(cx, |cx| {
            view.read_with(cx, |view, _| view.server_check.busy.is_none())
        });
        assert_eq!(fixture.requests.lock().unwrap().len() - before, 1);
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Servers);
            assert!(view.message.contains("500"));
            assert_eq!(
                view.sessions[&fixture.server.id].session().token,
                "renewed-token"
            );
        });
        fixture.mode.store(3, Ordering::SeqCst);
        let before = fixture.requests.lock().unwrap().len();
        cx.update(|_, cx| view.update(cx, |view, cx| view.choose_server(fixture.server.id, cx)));
        wait(cx, |_| fixture.requests.lock().unwrap().len() > before);
        cx.update(|_, cx| view.update(cx, |view, cx| view.choose_server(second.server.id, cx)));
        fixture.mode.store(0, Ordering::SeqCst);
        wait(cx, |cx| {
            view.read_with(cx, |view, _| view.server_check.busy.is_none())
        });
        wait(cx, |_| fixture.requests.lock().unwrap().len() >= before + 2);
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Home);
            assert_eq!(view.state.active_server, Some(second.server.id));
            assert_eq!(
                view.state
                    .queue
                    .entries
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>(),
                keys
            );
            assert!(!view.playing);
            assert_eq!(
                view.sessions[&fixture.server.id].session().token,
                "renewed-token"
            );
        });
        let saved = storage::load(&directory.join("state.json")).unwrap();
        assert_eq!(saved.servers.len(), 2);
        assert_eq!(saved.queue.entries.len(), 2);
        second.mode.store(3, Ordering::SeqCst);
        let before = second.requests.lock().unwrap().len();
        let removed_api = view.read_with(cx, |view, _| view.sessions[&second.server.id].clone());
        let late_session = removed_api.session();
        cx.update(|_, cx| view.update(cx, |view, cx| view.choose_server(second.server.id, cx)));
        wait(cx, |_| second.requests.lock().unwrap().len() > before);
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.remove_server(second.server.id, window, cx)
            })
        });
        cx.simulate_prompt_answer("Remove");
        wait(cx, |cx| {
            view.read_with(cx, |view, _| !view.sessions.contains_key(&second.server.id))
        });
        assert!(removed_api.is_signed_out());
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.restore_server_sessions(vec![(second.server.clone(), late_session)], cx);
                assert!(!view.sessions.contains_key(&second.server.id));
            })
        });
        second.mode.store(1, Ordering::SeqCst);
        wait(cx, |cx| {
            view.read_with(cx, |view, _| view.server_check.busy.is_none())
        });
        view.read_with(cx, |view, _| {
            assert_eq!(view.state.active_server, Some(fixture.server.id));
            assert_eq!(view.state.servers.len(), 1);
            assert_eq!(
                view.state
                    .queue
                    .entries
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>(),
                keys
            );
            assert_eq!(view.route, Route::Home);
        });
        fixture.mode.store(3, Ordering::SeqCst);
        let before = fixture.requests.lock().unwrap().len();
        cx.update(|_, cx| view.update(cx, |view, cx| view.verify_active_server(cx)));
        wait(cx, |_| fixture.requests.lock().unwrap().len() > before);
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.settings_import = Some(
                    feishin_core::migration::preview(
                        &view.state.settings,
                        &serde_json::to_vec(&view.state).unwrap(),
                    )
                    .unwrap(),
                );
                view.apply_settings_import(window, cx);
                assert_eq!(view.route, Route::Settings);
                assert!(view.server_check.busy.is_none());
            })
        });
        fixture.mode.store(0, Ordering::SeqCst);
        wait(cx, |_| fixture.requests.lock().unwrap().len() >= before + 2);
        let settled = Instant::now() + Duration::from_millis(100);
        wait(cx, |_| Instant::now() >= settled);
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Settings);
            assert!(view.message.starts_with("Imported "));
            assert_eq!(
                view.state
                    .queue
                    .entries
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>(),
                keys
            );
        });
        for change in ["options", "address", "signed-out"] {
            let api = fixture.api();
            cx.update(|window, cx| {
                view.update(cx, |view, cx| {
                    view.state.servers = vec![fixture.server.clone()];
                    view.sessions.insert(fixture.server.id, api.clone());
                    let mut imported = view.state.clone();
                    match change {
                        "signed-out" => {
                            imported.signed_out_servers.insert(fixture.server.id);
                        }
                        "address" => imported.servers[0].url = second.server.url.clone(),
                        _ => {
                            imported.servers[0].name = "Renamed".into();
                            imported.servers[0].options.music_folder_ids = vec!["current".into()];
                        }
                    }
                    let mut preview = feishin_core::migration::preview(
                        &view.state.settings,
                        &serde_json::to_vec(&imported).unwrap(),
                    )
                    .unwrap();
                    // Electron account imports merge definitions without replacing the queue.
                    preview.data.merge_servers = true;
                    preview.data.queue = None;
                    assert!(!preview.data.replaces_playback());
                    view.settings_import = Some(preview);
                    // Exercise transport state without opening an audio device.
                    view.playing = true;
                    view.loaded = true;
                    view.apply_settings_import(window, cx);
                    assert_eq!(view.playing, change == "options");
                    assert_eq!(view.loaded, change == "options");
                    assert_eq!(
                        view.state
                            .queue
                            .entries
                            .iter()
                            .map(|entry| entry.key)
                            .collect::<Vec<_>>(),
                        keys
                    );
                    if change == "options" {
                        assert_eq!(view.sessions[&fixture.server.id].server.name, "Renamed");
                        assert_eq!(
                            view.sessions[&fixture.server.id]
                                .server
                                .options
                                .music_folder_ids,
                            ["current"]
                        );
                        view.stop_playback(cx);
                    } else {
                        assert!(!view.sessions.contains_key(&fixture.server.id));
                        view.restore_server_sessions(
                            vec![(fixture.server.clone(), fixture.api().session())],
                            cx,
                        );
                        assert!(!view.sessions.contains_key(&fixture.server.id));
                    }
                })
            });
            assert_eq!(api.is_signed_out(), change != "options");
        }
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn discovery_results_stream_to_the_form_cancel_on_navigation_and_preserve_saved_servers(
        cx: &mut TestAppContext,
    ) {
        struct DiscoveryProbe(Entity<Feishin>);
        impl Render for DiscoveryProbe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.0
                    .update(cx, |view, cx| view.server_discovery_controls(cx))
            }
        }
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-discovery-ui-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let saved_id = uuid::Uuid::new_v4();
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = storage::State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            let view = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            view.update(cx, |view, _| {
                view.route = Route::Servers;
                view.state.servers.push(Server {
                    id: saved_id,
                    name: "Saved account".into(),
                    url: "http://localhost:8096".into(),
                    username: "fixture".into(),
                    kind: ServerType::Jellyfin,
                    legacy_auth: false,
                    options: Default::default(),
                });
                view.editing_server = Some(saved_id);
            });
            captured.replace(Some(view.clone()));
            let probe = cx.new(|_| DiscoveryProbe(view));
            Root::new(probe, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        cx.update(|_, cx| {
            view.update(cx, |view, _| {
                view.server_discovery.started = true;
                view.server_discovery.busy = true;
                view.server_discovery.receiver = Some(receiver);
            })
        });
        let found = feishin_core::server_discovery::Server {
            name: "Music room".into(),
            url: "https://localhost:8920/music".into(),
        };
        for _ in 0..2 {
            sender.send(DiscoveryEvent::Found(found.clone())).unwrap();
        }
        sender.send(DiscoveryEvent::Finished(Ok(()))).unwrap();
        cx.update(|_, cx| view.update(cx, |view, cx| view.poll_server_discovery(cx)));
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |view, _| view.server_discovery.servers.len()),
            1
        );
        let point = cx.debug_bounds("discovered-server-0").unwrap().center();
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(view.server_kind, ServerType::Jellyfin);
            assert_eq!(view.server_name.read(cx).value().as_ref(), found.name);
            assert_eq!(view.server_url.read(cx).value().as_ref(), found.url);
            assert_eq!(view.editing_server, None);
            assert_eq!(view.state.servers.len(), 1);
            assert_eq!(view.state.servers[0].id, saved_id);
            assert!(!view.server_discovery.busy);
        });
        let cancelled = view.read_with(cx, |view, _| view.server_discovery.cancelled.clone());
        let (stale, receiver) = std::sync::mpsc::channel();
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.server_discovery.busy = true;
                view.server_discovery.receiver = Some(receiver);
                view.route = Route::Settings;
                view.poll_server_discovery(cx);
            })
        });
        assert!(cancelled.load(std::sync::atomic::Ordering::Relaxed));
        assert!(stale.send(DiscoveryEvent::Found(found)).is_err());
        assert!(!view.read_with(cx, |view, _| view.server_discovery.busy));
        let cancellation = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut dropped = Discovery::default();
        dropped.cancelled = cancellation.clone();
        drop(dropped);
        assert!(cancellation.load(std::sync::atomic::Ordering::Relaxed));
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn tls_exception_mouse_toggle_updates_saved_servers_live_clients_and_future_login_forms(
        cx: &mut TestAppContext,
    ) {
        struct TlsProbe(Entity<Feishin>);
        impl Render for TlsProbe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.0.update(cx, |view, cx| view.server_tls_control(cx))
            }
        }
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-tls-control-{}", uuid::Uuid::new_v4()));
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "TLS".into(),
            url: "https://localhost:1".into(),
            username: "fixture".into(),
            kind: ServerType::Jellyfin,
            legacy_auth: false,
            options: Default::default(),
        };
        let api = Api::new(
            server.clone(),
            Session {
                token: "private-session".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = storage::State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            let probe = cx.new(|_| TlsProbe(app));
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.state.servers = vec![server.clone()];
                view.sessions.insert(server.id, api.clone());
                cx.notify();
            })
        });
        cx.run_until_parked();
        for enabled in [true, false] {
            let toggle = cx.debug_bounds("ignore-ssl").unwrap().center();
            cx.simulate_click(toggle, Modifiers::none());
            cx.run_until_parked();
            app.read_with(cx, |view, cx| {
                assert_eq!(view.state.settings.ignore_ssl, enabled);
                assert_eq!(view.state.servers[0].options.ignore_ssl, enabled);
                assert_eq!(view.form_server_options(cx).ignore_ssl, enabled);
                assert_eq!(view.sessions[&server.id].server.options.ignore_ssl, enabled);
                assert_eq!(view.sessions[&server.id].session().token, "private-session");
            });
            let saved = storage::load(&dir.join("state.json")).unwrap();
            assert_eq!(saved.settings.ignore_ssl, enabled);
            assert_eq!(saved.servers[0].options.ignore_ssl, enabled);
            assert!(
                !std::fs::read_to_string(dir.join("state.json"))
                    .unwrap()
                    .contains("private-session")
            );
            assert!(!api.is_signed_out());
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui::test]
    fn sign_out_mouse_revokes_only_the_selected_account_keeps_queue_and_survives_restart(
        cx: &mut TestAppContext,
    ) {
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-sign-out-{}", uuid::Uuid::new_v4()));
        let first = Server {
            id: uuid::Uuid::new_v4(),
            name: "First".into(),
            url: "http://127.0.0.1:1".into(),
            username: "first".into(),
            kind: ServerType::Jellyfin,
            legacy_auth: false,
            options: Default::default(),
        };
        let second = Server {
            id: uuid::Uuid::new_v4(),
            username: "second".into(),
            name: "Second".into(),
            ..first.clone()
        };
        let api = Api::new(
            first.clone(),
            Session {
                token: "private-first-token".into(),
                user_id: "first".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let other = Api::new(
            second.clone(),
            Session {
                token: "private-second-token".into(),
                user_id: "second".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = storage::State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.sidebar.items.clear();
            state.servers = vec![first.clone(), second.clone()];
            state.active_server = Some(first.id);
            state.queue.add(
                vec![
                    Item {
                        id: "first-song".into(),
                        server_id: first.id,
                        ..Default::default()
                    },
                    Item {
                        id: "second-song".into(),
                        server_id: second.id,
                        ..Default::default()
                    },
                ],
                Enqueue::End,
            );
            state.queue.current = Some(state.queue.entries[0].key);
            state.queue.position = 42.0;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            let probe = cx.new(|_| Probe(app));
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.sessions.insert(first.id, api.clone());
                view.sessions.insert(second.id, other.clone());
                view.route = Route::Home;
                view.items.push(Item {
                    id: "private-result".into(),
                    ..Default::default()
                });
                view.history.push(Route::Library(ItemKind::Song).into());
                view.playing = true;
                view.loaded = true;
                view.lyrics = feishin_core::lyrics::parse_lrc("[00:00.00]Private lyrics");
                cx.notify();
            })
        });
        cx.run_until_parked();
        let menu = cx.debug_bounds("application-menu").unwrap().center();
        cx.simulate_click(menu, Modifiers::none());
        cx.run_until_parked();
        for (index, label) in [
            first.name.clone(),
            app.read_with(cx, |view, _| view.tr("page.appMenu.logout")),
        ]
        .into_iter()
        .enumerate()
        {
            let target = cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                let bounds = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| {
                        node.role() == gpui::Role::MenuItem
                            && node.children().iter().any(|id| {
                                tree.nodes.iter().any(|(child_id, child)| {
                                    child_id == id && child.value() == Some(label.as_str())
                                })
                            })
                    })
                    .unwrap_or_else(|| panic!("Missing menu item: {label}"))
                    .1
                    .bounds()
                    .unwrap();
                let scale = f64::from(window.scale_factor());
                gpui::point(
                    gpui::px(((bounds.x0 + bounds.x1) / 2. / scale) as f32),
                    gpui::px(((bounds.y0 + bounds.y1) / 2. / scale) as f32),
                )
            });
            if index == 0 {
                cx.simulate_mouse_move(target, None, Modifiers::none());
            } else {
                cx.simulate_click(target, Modifiers::none());
            }
            cx.run_until_parked();
        }
        assert!(api.is_signed_out());
        assert!(!other.is_signed_out());
        app.read_with(cx, |view, _| {
            assert!(view.api().is_none());
            assert!(!view.sessions.contains_key(&first.id));
            assert!(view.sessions.contains_key(&second.id));
            assert_eq!(view.state.servers.len(), 2);
            assert!(view.state.signed_out_servers.contains(&first.id));
            assert_eq!(view.route, Route::Servers);
            assert_eq!(view.editing_server, Some(first.id));
            assert!(view.history.is_empty() && view.forward_history.is_empty());
            assert!(view.items.is_empty() && view.lyrics.is_empty());
            assert!(!view.playing && !view.loaded && !view.buffering);
            assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), queue);
        });
        let saved = storage::load(&dir.join("state.json")).unwrap();
        assert!(saved.active_server.is_none());
        assert!(saved.signed_out_servers.contains(&first.id));
        assert_eq!(serde_json::to_value(&saved.queue).unwrap(), queue);
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.read_with(cx, |view, _| view.signing_out.contains(&first.id)) {
            cx.run_until_parked();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let signed_in = Api::new(
            first.clone(),
            Session {
                token: "new-token".into(),
                user_id: "first".into(),
                ..Default::default()
            },
        )
        .unwrap();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.remember = true;
                view.finish_login(Ok((signed_in, None)), window, cx)
            })
        });
        assert!(!app.read_with(cx, |view, _| {
            view.state.signed_out_servers.contains(&first.id)
        }));
        assert!(
            !storage::load(&dir.join("state.json"))
                .unwrap()
                .signed_out_servers
                .contains(&first.id)
        );
        assert_eq!(
            app.read_with(cx, |view, _| view.api().unwrap().session().token),
            "new-token"
        );
        assert!(api.is_signed_out());
        let renewed = app.read_with(cx, |view, _| view.api().unwrap());
        cx.update(|window, cx| app.update(cx, |view, cx| view.open_command_palette(window, cx)));
        assert!(app.read_with(cx, |view, _| view.command_palette.is_some()));
        renewed.sign_out();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| view.expire_server_sessions(window, cx));
        });
        app.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Servers);
            assert!(view.command_palette.is_none());
            assert!(view.state.active_server.is_none());
            assert!(!view.sessions.contains_key(&first.id));
            assert!(view.sessions.contains_key(&second.id));
            assert!(view.state.signed_out_servers.contains(&first.id));
            assert!(!view.remember);
            assert_eq!(
                view.message,
                format!("First: {}", view.tr("error.sessionExpiredError"))
            );
            assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), queue);
        });
        assert!(!other.is_signed_out());
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.read_with(cx, |view, _| view.signing_out.contains(&first.id)) {
            cx.run_until_parked();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.message,
                format!("First: {}", view.tr("error.sessionExpiredError"))
            );
        });
        let saved = storage::load(&dir.join("state.json")).unwrap();
        assert!(saved.active_server.is_none());
        assert!(saved.signed_out_servers.contains(&first.id));
        assert_eq!(serde_json::to_value(&saved.queue).unwrap(), queue);
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.remember = false;
                let temporary = Api::new(
                    first.clone(),
                    Session {
                        token: "temporary-token".into(),
                        user_id: "first".into(),
                        ..Default::default()
                    },
                )
                .unwrap();
                view.finish_login(Ok((temporary, None)), window, cx);
                assert_eq!(view.api().unwrap().session().token, "temporary-token");
                assert!(view.state.signed_out_servers.contains(&first.id));
            })
        });
        assert!(
            storage::load(&dir.join("state.json"))
                .unwrap()
                .signed_out_servers
                .contains(&first.id)
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
