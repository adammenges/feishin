use crate::app::{Feishin, Route};
use feishin_core::{api::Api, model::ServerType};
use gpui::{prelude::*, *};
use gpui_component::{Disableable, Selectable, StyledExt, button::ButtonVariants};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
pub struct State {
    pub selected: bool,
    pub busy: bool,
    pub code: Option<String>,
    pub copied: bool,
    pub password_busy: bool,
    pub(crate) generation: u64,
    pub(crate) cancelled: Arc<AtomicBool>,
}

impl Drop for State {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Feishin {
    pub fn cancel_quick_connect(&mut self) {
        if self.quick_connect.password_busy {
            self.loading = false;
        }
        self.quick_connect.password_busy = false;
        self.quick_connect.cancelled.store(true, Ordering::Relaxed);
        self.quick_connect.generation = self.quick_connect.generation.wrapping_add(1);
        self.quick_connect.busy = false;
        self.quick_connect.code = None;
        self.quick_connect.copied = false;
    }

    fn quick_connect_current(&self, generation: u64) -> bool {
        self.quick_connect.generation == generation
            && self.quick_connect.busy
            && self.quick_connect.selected
            && self.server_kind == ServerType::Jellyfin
            && self.route == Route::Servers
    }

    pub fn start_quick_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading
            || self.quick_connect.busy
            || !self.quick_connect.selected
            || self.server_kind != ServerType::Jellyfin
            || self.route != Route::Servers
        {
            return;
        }
        self.cancel_quick_connect();
        let cancelled = Arc::new(AtomicBool::new(false));
        let api = match Api::new(self.server_from_form(cx), Default::default()) {
            Ok(api) => api.with_cancellation(cancelled.clone()),
            Err(error) => {
                self.message = error.to_string();
                cx.notify();
                return;
            }
        };
        self.quick_connect.cancelled = cancelled.clone();
        self.quick_connect.busy = true;
        self.message.clear();
        let generation = self.quick_connect.generation;
        let task = cx.background_executor().spawn(async move {
            let request = api.initiate_quick_connect()?;
            Ok::<_, anyhow::Error>((api, request))
        });
        cx.spawn_in(window, async move |this, cx| {
            let (api, mut request) = match task.await {
                Ok(result) => result,
                Err(error) => {
                    this.update_in(cx, |this, _, cx| {
                        this.quick_connect_failed(generation, error, cx);
                    })
                    .ok();
                    return;
                }
            };
            if !this
                .update_in(cx, |this, _, cx| {
                    if !this.quick_connect_current(generation) {
                        return false;
                    }
                    this.quick_connect.code = Some(request.code.clone());
                    cx.notify();
                    true
                })
                .unwrap_or(false)
            {
                return;
            }
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                if cancelled.load(Ordering::Relaxed) {
                    return;
                }
                let polling_api = api.clone();
                let (next_request, result) = cx
                    .background_executor()
                    .spawn(async move {
                        let result = polling_api.quick_connect_authenticated(&request);
                        (request, result)
                    })
                    .await;
                request = next_request;
                match result {
                    Ok(false) => continue,
                    Err(error) => {
                        this.update_in(cx, |this, _, cx| {
                            this.quick_connect_failed(generation, error, cx);
                        })
                        .ok();
                        return;
                    }
                    Ok(true) => {
                        let result = cx
                            .background_executor()
                            .spawn(async move { api.authenticate_quick_connect(&request) })
                            .await;
                        this.update_in(cx, |this, window, cx| {
                            if !this.quick_connect_current(generation) {
                                return;
                            }
                            match result {
                                Ok(mut api) => {
                                    // An existing account keeps its ID when Quick Connect supplies
                                    // a username that the form did not know in advance.
                                    if this.editing_server.is_none()
                                        && let Some(server) =
                                            this.state.servers.iter().find(|server| {
                                                server.kind == api.server.kind
                                                    && server.username == api.server.username
                                                    && feishin_core::api::normalize_url(&server.url)
                                                        .ok()
                                                        == feishin_core::api::normalize_url(
                                                            &api.server.url,
                                                        )
                                                        .ok()
                                            })
                                    {
                                        api.server.id = server.id;
                                    }
                                    this.quick_connect.code = None;
                                    this.finish_quick_connect(api, generation, window, cx);
                                }
                                Err(error) => {
                                    this.quick_connect_failed(generation, error, cx);
                                }
                            }
                        })
                        .ok();
                        return;
                    }
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn finish_quick_connect(
        &mut self,
        api: Api,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.signing_out.contains(&api.server.id) {
            self.quick_connect_failed(generation, anyhow::anyhow!("Finishing sign out..."), cx);
            return;
        }
        let remember = self.remember;
        let cancelled = self.quick_connect.cancelled.clone();
        let task = cx.background_executor().spawn(async move {
            anyhow::ensure!(
                !cancelled.load(Ordering::Relaxed),
                "Quick Connect cancelled"
            );
            crate::servers::prepare_login(api.without_cancellation(), remember, Some(&cancelled))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                if !this.quick_connect_current(generation) {
                    return;
                }
                this.cancel_quick_connect();
                this.finish_login(result, window, cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn quick_connect_failed(
        &mut self,
        generation: u64,
        error: anyhow::Error,
        cx: &mut Context<Self>,
    ) {
        if !self.quick_connect_current(generation) {
            return;
        }
        self.cancel_quick_connect();
        log::warn!("Jellyfin Quick Connect failed: {error}");
        self.message = error.to_string();
        cx.notify();
    }

    pub fn jellyfin_sign_in_method(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_2()
            .child(self.tr("form.addServer.input_jellyfinSignInMethod"))
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .children([false, true].into_iter().map(|quick| {
                        crate::styles::button(
                            if quick {
                                "quick-connect-method"
                            } else {
                                "password-method"
                            },
                            cx,
                        )
                        .debug_selector(move || {
                            if quick {
                                "quick-connect-method".into()
                            } else {
                                "password-method".into()
                            }
                        })
                        .selected(self.quick_connect.selected == quick)
                        .label(self.tr(if quick {
                            "form.addServer.input_jellyfinSignInMethodQuickConnect"
                        } else {
                            "form.addServer.input_jellyfinSignInMethodPassword"
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.cancel_quick_connect();
                            this.quick_connect.selected = quick;
                            cx.notify();
                        }))
                    })),
            )
            .into_any_element()
    }

    pub fn quick_connect_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut panel = div().v_flex().gap_2();
        if let Some(code) = &self.quick_connect.code {
            let code = code.clone();
            panel = panel
                .child(self.tr("form.addServer.input_jellyfinQuickConnectCode"))
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(div().text_xl().child(code.clone()))
                        .child(
                            crate::styles::button("quick-connect-copy", cx)
                                .debug_selector(|| "quick-connect-copy".into())
                                .label(self.tr(if self.quick_connect.copied {
                                    "form.addServer.input_jellyfinQuickConnectCodeCopied"
                                } else {
                                    "form.addServer.input_jellyfinQuickConnectCodeCopy"
                                }))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                                    this.quick_connect.copied = true;
                                    let generation = this.quick_connect.generation;
                                    cx.spawn(async move |this, cx| {
                                        cx.background_executor()
                                            .timer(Duration::from_secs(2))
                                            .await;
                                        this.update(cx, |this, cx| {
                                            if this.quick_connect.generation == generation {
                                                this.quick_connect.copied = false;
                                                cx.notify();
                                            }
                                        })
                                        .ok();
                                    })
                                    .detach();
                                    cx.notify();
                                })),
                        ),
                );
            let server_url = if self.server_options.prefer_remote_url {
                &self.server_remote_url
            } else {
                &self.server_url
            };
            if let Ok(url) = feishin_core::api::normalize_url(&server_url.read(cx).value()) {
                let url = format!("{url}/web/#/quickconnect");
                panel = panel.child(
                    crate::styles::button("quick-connect-open", cx)
                        .label(self.tr("form.addServer.input_jellyfinQuickConnectOpen"))
                        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url))),
                );
            }
            panel = panel.child(self.tr("form.addServer.input_jellyfinQuickConnectWaiting"));
        } else {
            panel = panel.child(
                crate::styles::button("quick-connect-start", cx)
                    .primary()
                    .debug_selector(|| "quick-connect-start".into())
                    .label(self.tr(if self.quick_connect.busy {
                        "Connecting..."
                    } else {
                        "form.addServer.input_jellyfinQuickConnect"
                    }))
                    .disabled(self.quick_connect.busy || self.loading)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.start_quick_connect(window, cx)),
                    ),
            );
        }
        if self.quick_connect.busy {
            panel = panel.child(
                crate::styles::button("quick-connect-cancel", cx)
                    .ghost()
                    .debug_selector(|| "quick-connect-cancel".into())
                    .label(self.tr("Cancel"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cancel_quick_connect();
                        cx.notify();
                    })),
            );
        }
        panel.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Arc, AtomicBool, Duration, Feishin, Ordering, Route};
    use feishin_core::{
        model::{Server, ServerType},
        storage,
    };
    use gpui::{
        AppContext, Context, Entity, Modifiers, Render, TestAppContext, Window, div, prelude::*,
    };
    use gpui_component::{Root, StyledExt};
    use serde_json::json;
    use std::{
        cell::RefCell,
        io::{Read, Write},
        net::TcpListener,
        rc::Rc,
        thread,
        time::Instant,
    };

    struct Probe(Entity<Feishin>);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0.update(cx, |view, cx| {
                div()
                    .v_flex()
                    .gap_2()
                    .child(view.jellyfin_sign_in_method(cx))
                    .child(view.quick_connect_controls(cx))
            })
        }
    }

    #[gpui::test]
    fn quick_connect_mouse_copy_cancel_edits_poll_and_authorization_preserve_the_account(
        cx: &mut TestAppContext,
    ) {
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let approved = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let (stop, authorize, requests) = (done.clone(), approved.clone(), calls.clone());
        let worker = thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = vec![];
                let mut buffer = [0; 4096];
                loop {
                    let count = socket.read(&mut buffer).unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]);
                        let length = header
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let request = String::from_utf8_lossy(&bytes).into_owned();
                let path = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                requests.lock().unwrap().push(request.clone());
                let body = match path.split('?').next().unwrap() {
                    "/QuickConnect/Initiate" => json!({"Code":"123456", "Secret":"fixture-authorization-secret"}),
                    "/QuickConnect/Connect" => json!({"Authenticated":authorize.load(Ordering::Relaxed)}),
                    "/Users/AuthenticateWithQuickConnect" => json!({"AccessToken":"fixture-auth-token", "User":{"Id":"authorized-user", "Name":"Fixture Account", "Policy":{"IsAdministrator":true}}}),
                    "/Users/AuthenticateByName" => json!({"AccessToken":"late-password-token", "User":{"Id":"late-user", "Name":"Fixture Account"}}),
                    "/System/Info/Public" => json!({"Version":"10.9.11"}),
                    _ => json!({"Items":[], "TotalRecordCount":0}),
                }.to_string();
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).ok();
            }
        });
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-quick-connect-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = storage::State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.servers.push(Server {
                id,
                name: "Saved account".into(),
                url: address.clone(),
                username: "Fixture Account".into(),
                kind: ServerType::Jellyfin,
                legacy_auth: false,
                options: Default::default(),
            });
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.route = Route::Servers;
                view.edit_server(None, window, cx);
                view.server_kind = ServerType::Jellyfin;
                view.server_url
                    .update(cx, |input, cx| input.set_value(address.clone(), window, cx));
                view.server_name
                    .update(cx, |input, cx| input.set_value("Authorized", window, cx));
            });
            captured.replace(Some(app.clone()));
            let probe = cx.new(|_| Probe(app));
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        let click = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
            cx.run_until_parked();
            let center = cx.debug_bounds(selector).unwrap().center();
            cx.simulate_click(center, Modifiers::none());
            cx.run_until_parked();
        };
        click(cx, "quick-connect-method");
        for reason in 0..5 {
            click(cx, "quick-connect-start");
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.read_with(cx, |view, _| view.quick_connect.code.is_none()) {
                cx.run_until_parked();
                assert!(
                    Instant::now() < deadline,
                    "Quick Connect did not supply its code"
                );
                thread::sleep(Duration::from_millis(5));
            }
            click(cx, "quick-connect-copy");
            assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "123456");
            assert!(app.read_with(cx, |view, _| view.quick_connect.copied));
            let generation = app.read_with(cx, |view, _| view.quick_connect.generation);
            if reason == 0 {
                click(cx, "quick-connect-cancel");
            } else if reason == 1 {
                click(cx, "password-method");
            } else {
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        if reason == 2 {
                            view.server_url.update(cx, |input, cx| {
                                input.set_value(format!("{address}/changed"), window, cx)
                            });
                        } else if reason == 3 {
                            view.edit_server(None, window, cx);
                        } else {
                            view.navigate(Route::Home, cx);
                        }
                    })
                });
            }
            cx.run_until_parked();
            cx.update(|_, cx| {
                app.update(cx, |view, cx| {
                    assert!(!view.quick_connect.busy && view.quick_connect.code.is_none());
                    view.quick_connect_failed(generation, anyhow::anyhow!("stale request"), cx);
                    assert!(!view.message.contains("stale request"));
                    assert!(view.sessions.is_empty());
                })
            });
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.route = Route::Servers;
                    view.server_kind = ServerType::Jellyfin;
                    view.server_url
                        .update(cx, |input, cx| input.set_value(address.clone(), window, cx));
                })
            });
            click(cx, "quick-connect-method");
        }
        click(cx, "quick-connect-start");
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.read_with(cx, |view, _| view.quick_connect.code.is_none()) {
            cx.run_until_parked();
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        cx.executor().advance_clock(Duration::from_secs(5));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !calls
            .lock()
            .unwrap()
            .iter()
            .any(|request| request.starts_with("GET /QuickConnect/Connect?"))
        {
            cx.run_until_parked();
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        assert!(app.read_with(cx, |view, _| view.quick_connect.busy
            && view.sessions.is_empty()));
        approved.store(true, Ordering::Relaxed);
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(5));
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.read_with(cx, |view, _| view.sessions.is_empty()) {
            cx.run_until_parked();
            assert!(
                Instant::now() < deadline,
                "Authorized account was not installed"
            );
            thread::sleep(Duration::from_millis(5));
        }
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.servers.len(), 1);
            assert_eq!(view.state.active_server, Some(id));
            assert_eq!(view.api().unwrap().server.username, "Fixture Account");
            assert_eq!(view.api().unwrap().session().user_id, "authorized-user");
            assert!(view.api().unwrap().session().is_admin);
            assert_eq!(view.route, Route::Home);
            assert!(!view.quick_connect.busy && view.quick_connect.code.is_none());
            assert!(
                view.api()
                    .unwrap()
                    .stream_url(
                        &feishin_core::model::Item {
                            id: "song".into(),
                            server_id: id,
                            ..Default::default()
                        },
                        &view.state.settings
                    )
                    .unwrap()
                    .contains("fixture-auth-token")
            );
        });
        let saved = std::fs::read_to_string(dir.join("state.json")).unwrap();
        assert!(
            !saved.contains("fixture-auth-token")
                && !saved.contains("fixture-authorization-secret")
        );
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Servers;
                view.edit_server(Some(id), window, cx);
                view.password.update(cx, |input, cx| {
                    input.set_value("temporary-password", window, cx)
                });
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| app.update(cx, |view, cx| view.login(window, cx)));
        let cancelled = app.read_with(cx, |view, _| view.quick_connect.cancelled.clone());
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.server_url.update(cx, |input, cx| {
                    input.set_value(format!("{address}/edited"), window, cx)
                });
            })
        });
        cx.run_until_parked();
        assert!(cancelled.load(Ordering::Relaxed));
        assert!(!app.read_with(cx, |view, _| view.loading
            || view.quick_connect.password_busy));
        let deadline = Instant::now() + Duration::from_secs(5);
        while Arc::strong_count(&cancelled) > 2 {
            cx.run_until_parked();
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.api().unwrap().session().token, "fixture-auth-token");
            assert_eq!(view.route, Route::Servers);
            assert_eq!(view.state.servers.len(), 1);
        });
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.server_url
                    .update(cx, |input, cx| input.set_value(address.clone(), window, cx));
                view.username.update(cx, |input, cx| {
                    input.set_value("Another account", window, cx)
                });
                view.quick_connect.selected = true;
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| app.update(cx, |view, cx| view.save_server_options(window, cx)));
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.read_with(cx, |view, _| view.quick_connect.code.is_none()) {
            cx.run_until_parked();
            assert!(
                Instant::now() < deadline,
                "Saving an account edit did not start the chosen Quick Connect method"
            );
            thread::sleep(Duration::from_millis(5));
        }
        cx.update(|_, cx| app.update(cx, |view, _| view.cancel_quick_connect()));
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request.starts_with("POST /Users/AuthenticateByName "))
                .count(),
            1
        );
        done.store(true, Ordering::Relaxed);
        worker.join().unwrap();
        std::fs::remove_dir_all(dir).ok();
    }
}
