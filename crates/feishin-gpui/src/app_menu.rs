use crate::{
    app::{Feishin, Quit, Route},
    shortcuts::RunShortcut,
};
use feishin_core::{
    i18n,
    queue::Repeat,
    shortcuts::{Action, Bindings},
    storage::Settings,
};
use gpui::{
    AnyWindowHandle, App, Context, Global, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType,
    WeakEntity, Window,
};
use gpui_component::input;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum Command {
    About,
    Settings,
    Servers,
    Private,
    CreatePlaylist,
    Sidebar,
    Show,
    Close,
    Hide,
    HideOthers,
    ShowAll,
    Minimize,
    ReleaseNotes,
    Learn,
    Documentation,
    Community,
    Issues,
    Reload,
    Restart,
    Palette,
    Back,
    Forward,
    ExpandSidebar,
    CollapseSidebar,
    PrivateOn,
    PrivateOff,
    Inspector,
    PanelHorizontal,
    PanelVertical,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, gpui::Action)]
#[action(namespace = feishin, no_json)]
pub struct RunMenu {
    pub command: Command,
}

gpui::actions!(app_menu, [Unavailable]);

#[derive(Clone)]
struct MainWindow {
    view: WeakEntity<Feishin>,
    handle: AnyWindowHandle,
}
impl Global for MainWindow {}

#[derive(Clone, PartialEq, Eq)]
pub struct Signature {
    language: String,
    playing: bool,
    shuffle: bool,
    repeat: bool,
    private: bool,
    collapsed: bool,
    input: bool,
    shortcuts: Bindings,
}
impl Signature {
    fn new(settings: &Settings, playing: bool, shuffle: bool, repeat: bool, input: bool) -> Self {
        Self {
            language: settings.language.clone(),
            playing,
            shuffle,
            repeat,
            private: settings.private_mode,
            collapsed: settings.sidebar.collapsed,
            input,
            shortcuts: settings.shortcuts.clone(),
        }
    }
    fn text(&self, value: &str) -> String {
        i18n::text(&self.language, value)
    }
    fn menu(&self, name: &str, items: Vec<MenuItem>) -> Menu {
        Menu {
            name: self.text(name).into(),
            items,
        }
    }
    fn item(&self, label: &str, command: Command) -> MenuItem {
        MenuItem::action(self.text(label), RunMenu { command })
    }
    fn playback(&self, label: &str, action: Action) -> MenuItem {
        let key = self
            .shortcuts
            .get(&action)
            .map(|binding| binding.key.as_str())
            .unwrap_or("");
        let (key, action) = if key.is_empty() && matches!(action, Action::Play | Action::Pause) {
            (
                self.shortcuts
                    .get(&Action::PlayPause)
                    .map(|binding| binding.key.as_str())
                    .unwrap_or(""),
                Action::PlayPause,
            )
        } else {
            (key, action)
        };
        if self.input && typing_sensitive(key) {
            MenuItem::action(self.text(label), Unavailable)
        } else {
            MenuItem::action(self.text(label), RunShortcut { action })
        }
    }
    #[cfg(any(target_os = "macos", test))]
    fn checked(&self, label: &str) -> bool {
        [
            ("Private Session", self.private),
            ("Collapse Sidebar", self.collapsed),
            ("Shuffle", self.shuffle),
            ("Repeat", self.repeat),
        ]
        .iter()
        .any(|(name, checked)| *checked && self.text(name) == label)
    }
    fn menus(&self) -> Vec<Menu> {
        use Command::*;
        vec![
            self.menu(
                "Feishin",
                vec![
                    self.item("About Feishin", About),
                    MenuItem::separator(),
                    self.item("Settings...", Settings),
                    self.item("Manage Servers...", Servers),
                    self.item("Private Session", Private),
                    MenuItem::separator(),
                    MenuItem::os_submenu(self.text("Services"), SystemMenuType::Services),
                    MenuItem::separator(),
                    self.item("Hide Feishin", Hide),
                    self.item("Hide Others", HideOthers),
                    self.item("Show All", ShowAll),
                    MenuItem::separator(),
                    MenuItem::action(self.text("Quit"), Quit),
                ],
            ),
            self.menu(
                "File",
                vec![
                    self.item("Create Playlist...", CreatePlaylist),
                    MenuItem::separator(),
                    self.item("Close", Close),
                ],
            ),
            self.menu(
                "Edit",
                vec![
                    MenuItem::os_action(self.text("Undo"), input::Undo, OsAction::Undo),
                    MenuItem::os_action(self.text("Redo"), input::Redo, OsAction::Redo),
                    MenuItem::separator(),
                    MenuItem::os_action(self.text("Cut"), input::Cut, OsAction::Cut),
                    MenuItem::os_action(self.text("Copy"), input::Copy, OsAction::Copy),
                    MenuItem::os_action(self.text("Paste"), input::Paste, OsAction::Paste),
                    MenuItem::os_action(
                        self.text("Select All"),
                        input::SelectAll,
                        OsAction::SelectAll,
                    ),
                ],
            ),
            self.menu(
                "View",
                vec![
                    MenuItem::action(
                        self.text("Command Palette..."),
                        RunShortcut {
                            action: Action::GlobalSearch,
                        },
                    ),
                    self.item("Collapse Sidebar", Sidebar),
                    MenuItem::separator(),
                    MenuItem::action(
                        self.text("Toggle Full Screen"),
                        RunShortcut {
                            action: Action::ToggleFullscreenWindow,
                        },
                    ),
                    MenuItem::submenu(self.menu(
                        "Developer",
                        vec![
                            self.item("Reload", Reload),
                            MenuItem::action(
                                self.text("Toggle Developer Tools"),
                                gpui_component::ToggleInspector,
                            ),
                        ],
                    )),
                ],
            ),
            self.menu(
                "Playback",
                vec![
                    self.playback(
                        if self.playing { "Pause" } else { "Play" },
                        if self.playing {
                            Action::Pause
                        } else {
                            Action::Play
                        },
                    ),
                    MenuItem::separator(),
                    self.playback("Next", Action::Next),
                    self.playback("Previous", Action::Previous),
                    self.playback("Seek Forward", Action::SkipForward),
                    self.playback("Seek Backward", Action::SkipBackward),
                    MenuItem::separator(),
                    self.playback("Shuffle", Action::ToggleShuffle),
                    self.playback("Repeat", Action::ToggleRepeat),
                    MenuItem::separator(),
                    self.playback("Stop", Action::Stop),
                    MenuItem::separator(),
                    self.playback("Volume Up", Action::VolumeUp),
                    self.playback("Volume Down", Action::VolumeDown),
                ],
            ),
            self.menu(
                "Window",
                vec![
                    self.item("Minimize", Minimize),
                    self.item("Show Feishin", Show),
                ],
            ),
            self.menu(
                "Help",
                vec![
                    self.item("Learn More", Learn),
                    self.item("Documentation", Documentation),
                    self.item("Community Discussions", Community),
                    self.item("Search Issues", Issues),
                    MenuItem::separator(),
                    MenuItem::action(
                        format!("Version {}", env!("CARGO_PKG_VERSION")),
                        RunMenu {
                            command: ReleaseNotes,
                        },
                    ),
                ],
            ),
        ]
    }
}

fn typing_sensitive(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    let Ok(key) = gpui::Keystroke::parse(key) else {
        return false;
    };
    let function = key
        .key
        .strip_prefix('f')
        .and_then(|value| value.parse::<u8>().ok())
        .is_some_and(|value| (1..=24).contains(&value));
    !function
        && !key.key.starts_with("media")
        && !key.key.starts_with("volume")
        && !key.modifiers.platform
        && !key.modifiers.control
        && !key.modifiers.alt
}

pub fn reopen(cx: &mut App) {
    if let Some(main) = cx.try_global::<MainWindow>().cloned() {
        cx.defer(move |cx| {
            main.handle
                .update(cx, |_, window, cx| {
                    main.view
                        .update(cx, |view, cx| {
                            view.tray_command(crate::tray::Command::Show, window, cx)
                        })
                        .ok();
                })
                .ok();
        });
    }
}

impl Feishin {
    pub fn init_app_menu(&self, window: &Window, cx: &mut Context<Self>) {
        let main = MainWindow {
            view: cx.weak_entity(),
            handle: window.window_handle(),
        };
        cx.set_global(main.clone());
        let shortcuts = main.clone();
        gpui::App::on_action(cx, move |action: &RunShortcut, cx| {
            let main = shortcuts.clone();
            let action = action.action;
            cx.defer(move |cx| {
                main.handle
                    .update(cx, |_, window, cx| {
                        main.view
                            .update(cx, |view, cx| view.run_shortcut(action, window, cx))
                            .ok();
                    })
                    .ok();
            });
        });
        gpui::App::on_action(cx, move |action: &RunMenu, cx| {
            let main = main.clone();
            let command = action.command;
            cx.defer(move |cx| {
                main.handle
                    .update(cx, |_, window, cx| {
                        main.view
                            .update(cx, |view, cx| view.run_menu(command, window, cx))
                            .ok();
                    })
                    .ok();
            });
        });
        cx.bind_keys([
            KeyBinding::new(
                "secondary-,",
                RunMenu {
                    command: Command::Settings,
                },
                None,
            ),
            KeyBinding::new(
                "secondary-w",
                RunMenu {
                    command: Command::Close,
                },
                None,
            ),
        ]);
        if cfg!(target_os = "macos") {
            cx.bind_keys([
                KeyBinding::new(
                    "cmd-h",
                    RunMenu {
                        command: Command::Hide,
                    },
                    None,
                ),
                KeyBinding::new(
                    "cmd-alt-h",
                    RunMenu {
                        command: Command::HideOthers,
                    },
                    None,
                ),
                KeyBinding::new(
                    "cmd-m",
                    RunMenu {
                        command: Command::Minimize,
                    },
                    None,
                ),
            ]);
        }
    }
    fn configure_menu_libraries(
        &mut self,
        id: uuid::Uuid,
        libraries: Vec<String>,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let mut server = self
            .state
            .servers
            .iter()
            .find(|server| server.id == id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Server is no longer available"))?;
        server.options.music_folder_ids = libraries;
        let api = self
            .sessions
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("Sign in to this server again"))?;
        anyhow::ensure!(!api.is_signed_out(), "Sign in to this server again");
        let configured = api.reconfigure(server.clone())?;
        self.sessions.insert(id, configured);
        *self
            .state
            .servers
            .iter_mut()
            .find(|server| server.id == id)
            .unwrap() = server;
        self.save();
        cx.notify();
        Ok(())
    }

    pub fn select_menu_library(
        &mut self,
        id: uuid::Uuid,
        folder: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        if self.state.active_server != Some(id) {
            return;
        }
        let Some(server) = self.state.servers.iter().find(|server| server.id == id) else {
            return;
        };
        let mut libraries = server.options.music_folder_ids.clone();
        if let Some(folder) = folder {
            if server.kind == feishin_core::model::ServerType::Navidrome {
                if libraries.iter().any(|value| value == folder) {
                    libraries.retain(|value| value != folder);
                } else {
                    libraries.push(folder.into());
                }
            } else if libraries.first().is_some_and(|value| value == folder) {
                libraries.clear();
            } else {
                libraries = vec![folder.into()];
            }
        } else {
            libraries.clear();
        }
        match self.configure_menu_libraries(id, libraries, cx) {
            Ok(()) => {
                self.reset_playlists();
                self.reload(false, cx);
            }
            Err(error) => {
                log::warn!("Cannot select music library: {error}");
                self.message = error.to_string();
                cx.notify();
            }
        }
    }

    pub(crate) fn application_menu_icon(&self, size: gpui::Pixels) -> gpui::AnyElement {
        use gpui::{IntoElement, ParentElement, prelude::*};
        use gpui_component::Sizable;
        if self.library_scan.server == self.state.active_server && self.library_scan.status.scanning
        {
            gpui::div()
                .debug_selector(|| "application-menu-scanning".into())
                .child(
                    gpui_component::spinner::Spinner::new()
                        .icon(gpui_component::Icon::default().path("native-icons/spinner.svg"))
                        .with_size(size)
                        .animation(std::time::Duration::from_millis(1500), gpui::linear),
                )
                .into_any_element()
        } else {
            gpui_component::Icon::default()
                .path("native-icons/menu.svg")
                .with_size(size)
                .into_any_element()
        }
    }

    pub fn application_menu_button(
        &self,
        width: Option<f32>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        use gpui::{ParentElement, Styled, prelude::*};
        use gpui_component::{
            ActiveTheme, Sizable,
            button::{ButtonVariant, ButtonVariants},
        };
        let button = crate::styles::button("application-menu", cx)
            .debug_selector(|| "application-menu".into())
            .ghost()
            .small()
            .child(self.application_menu_icon(cx.theme().font_size))
            .tooltip(self.tr("common.menu"))
            .when_some(width, |button, width| {
                button
                    .with_variant(ButtonVariant::Secondary)
                    .w(gpui::px(width))
                    .h(cx.theme().font_size * 2.25)
                    .min_w_0()
                    .p_0()
                    .flex_shrink_0()
            });
        self.application_menu_for_button(button, false, cx)
    }

    pub(crate) fn application_menu_for_button(
        &self,
        button: gpui_component::button::Button,
        beside: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        use gpui::{IntoElement, ParentElement, Styled, prelude::*};
        use gpui_component::{
            IconName, Selectable, Sizable, StyledExt,
            button::ButtonVariants,
            menu::{DropdownMenu, PopupMenuItem},
        };
        let view = cx.weak_entity();
        let language = self.state.settings.language.clone();
        let collapsed = self.state.settings.sidebar.collapsed;
        let private = self.state.settings.private_mode;
        let side_queue = self.state.settings.panels.kind == "sideQueue";
        let active = self.state.active_server;
        let servers = self.state.servers.clone();
        let current = servers
            .iter()
            .find(|server| Some(server.id) == active)
            .cloned();
        let api = self.api();
        let can_scan = self.can_scan_library();
        let update = self
            .updates
            .available
            .as_ref()
            .map(|available| (available.version.clone(), available.page.clone()));
        button
            .dropdown_menu(move |mut menu, window, cx| {
                let item = |key: &str, command: Command| {
                    let view = view.clone();
                    PopupMenuItem::new(i18n::text(&language, key)).on_click(move |_, window, cx| {
                        let view = view.clone();
                        window.defer(cx, move |window, cx| {
                            view.update(cx, |this, cx| this.run_menu(command, window, cx))
                                .ok();
                        });
                    })
                };
                if let Some(current) = &current {
                    let servers = servers.clone();
                    let view = view.clone();
                    let current_id = current.id;
                    let language = language.clone();
                    let api = api.clone();
                    menu = menu.submenu(current.name.clone(), window, cx, move |mut submenu, _, cx| {
                        for server in &servers {
                            let id = server.id;
                            let view = view.clone();
                            submenu = submenu.item(PopupMenuItem::new(server.name.clone()).checked(id == current_id).on_click(move |_, window, cx| {
                                let view = view.clone();
                                window.defer(cx, move |window,cx| { view.update(cx, |this,cx| {
                                    if this.sessions.get(&id).is_none_or(|api| api.is_signed_out()) {
                                        this.edit_server(Some(id),window,cx);
                                        this.navigate(Route::Servers,cx);
                                    } else if let Err(error) = this.configure_menu_libraries(id,vec![],cx) {
                                        log::warn!("Cannot change account library selection: {error}"); this.message = error.to_string(); cx.notify();
                                    } else { this.choose_server(id,cx); }
                                }).ok(); });
                            }));
                        }
                        let manage = view.clone();
                        submenu = submenu.separator().item(PopupMenuItem::new(i18n::text(&language,"page.appMenu.manageServers")).on_click(move |_,window,cx| {
                            let view = manage.clone(); window.defer(cx, move |window,cx| { view.update(cx, |this,cx| this.run_menu(Command::Servers,window,cx)).ok(); });
                        }));
                        if can_scan {
                            let view = view.clone();
                            submenu = submenu.item(PopupMenuItem::new(i18n::text(&language,"page.appMenu.rescanLibrary")).on_click(move |_,window,cx| {
                                let view = view.clone(); window.defer(cx, move |_,cx| { view.update(cx, |this,cx| this.start_library_scan(cx)).ok(); });
                            }));
                        }
                        let logout = view.clone();
                        submenu = submenu.item(PopupMenuItem::new(i18n::text(&language,"page.appMenu.logout")).on_click(move |_,window,cx| {
                            let view = logout.clone(); window.defer(cx, move |window,cx| { view.update(cx, |this,cx| this.sign_out_server(current_id,window,cx)).ok(); });
                        }));
                        if let Some(api) = api.clone() {
                            let query = feishin_core::model::ListQuery { kind:feishin_core::model::ItemKind::Folder, ..Default::default() };
                            let address = api.server.url.clone();
                            let username = api.server.username.clone();
                            let options = api.server.options.clone();
                            let task = cx.background_executor().spawn(async move { api.all_items(query) });
                            let view = view.clone();
                            let language = language.clone();
                            cx.spawn(async move |popup,cx| {
                                let result = task.await;
                                popup.update(cx, |popup,cx| {
                                    let selected = view.read_with(cx, |this,_| {
                                        (this.state.active_server == Some(current_id)
                                            && this.sessions.get(&current_id).is_some_and(|api| !api.is_signed_out() && api.server.url==address && api.server.username==username
                                                && api.server.options.remote_url==options.remote_url && api.server.options.prefer_remote_url==options.prefer_remote_url
                                                && api.server.options.ignore_ssl==options.ignore_ssl))
                                            .then(|| this.state.servers.iter().find(|server| server.id==current_id).map(|server| server.options.music_folder_ids.clone()).unwrap_or_default())
                                    }).ok().flatten();
                                    let Some(selected) = selected else { return; };
                                    let folders = match result {
                                        Ok(folders) => folders,
                                        Err(error) => { log::debug!("Application menu libraries unavailable: {error}"); return; }
                                    };
                                    if folders.is_empty() { return; }
                                    let clear = view.clone();
                                    let mut rows = vec![PopupMenuItem::separator(),PopupMenuItem::label(i18n::text(&language,"page.appMenu.selectMusicFolder")),
                                        PopupMenuItem::new(i18n::text(&language,"common.none")).checked(selected.is_empty()).on_click(move |_,window,cx| {
                                            let view = clear.clone(); window.defer(cx, move |_,cx| { view.update(cx, |this,cx| this.select_menu_library(current_id,None,cx)).ok(); });
                                        })];
                                    for folder in folders {
                                        let view = view.clone();
                                        let id = folder.id;
                                        rows.push(PopupMenuItem::new(folder.name).checked(selected.contains(&id)).on_click(move |_,window,cx| {
                                            let view = view.clone(); let id = id.clone(); window.defer(cx, move |_,cx| { view.update(cx, |this,cx| this.select_menu_library(current_id,Some(&id),cx)).ok(); });
                                        }));
                                    }
                                    popup.extend_items(rows,cx);
                                }).ok();
                            }).detach();
                        }
                        submenu
                    }).separator();
                }
                menu = menu
                    .item(item("page.appMenu.commandPalette", Command::Palette))
                    .separator();
                if collapsed {
                    menu = menu
                        .item(item("page.appMenu.goBack", Command::Back))
                        .item(item("page.appMenu.goForward", Command::Forward));
                }
                menu = menu
                    .item(item(
                        if collapsed {
                            "page.appMenu.expandSidebar"
                        } else {
                            "page.appMenu.collapseSidebar"
                        },
                        if collapsed {
                            Command::ExpandSidebar
                        } else {
                            Command::CollapseSidebar
                        },
                    ))
                    .separator()
                    .item(item("page.appMenu.settings", Command::Settings))
                    .item(item(
                        if private {
                            "page.appMenu.privateModeOff"
                        } else {
                            "page.appMenu.privateModeOn"
                        },
                        if private {
                            Command::PrivateOff
                        } else {
                            Command::PrivateOn
                        },
                    ))
                    .separator();
                let version = i18n::translate_with(
                    &language,
                    "page.appMenu.version",
                    &serde_json::json!({"version":env!("CARGO_PKG_VERSION")}),
                )
                .unwrap();
                let view_for_version = view.clone();
                menu = menu
                    .item(PopupMenuItem::new(version).on_click(move |_, window, cx| {
                        let view = view_for_version.clone();
                        window.defer(cx, move |window, cx| {
                            view.update(cx, |this, cx| {
                                this.run_menu(Command::ReleaseNotes, window, cx)
                            })
                            .ok();
                        });
                    }))
                    .item(item("page.appMenu.openBrowserDevtools", Command::Inspector))
                    .item(
                        PopupMenuItem::new(i18n::text(&language, "page.appMenu.quit"))
                            .on_click(|_, window, cx| window.dispatch_action(Box::new(Quit), cx)),
                    );
                if let Some((version, page)) = &update {
                    let label = format!(
                        "{}: v{}",
                        i18n::text(&language, "common.newVersionAvailable"),
                        version.trim_start_matches('v')
                    );
                    let page = page.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label).on_click(move |_, _, cx| cx.open_url(&page)),
                    );
                }
                if side_queue {
                    let view = view.clone();
                    let menu_view = cx.weak_entity();
                    let language = language.clone();
                    menu = menu.separator().item(PopupMenuItem::element(move |_, cx| {
                        let layout = view
                            .read_with(cx, |this, _| this.state.settings.panels.layout.clone())
                            .unwrap_or_default();
                        gpui::div().h_flex().w_full().gap_2().children(
                            [
                                (
                                    "horizontal",
                                    "setting.sidePlayQueueLayout_optionHorizontal",
                                    IconName::PanelRight,
                                    Command::PanelHorizontal,
                                ),
                                (
                                    "vertical",
                                    "setting.sidePlayQueueLayout_optionVertical",
                                    IconName::PanelBottom,
                                    Command::PanelVertical,
                                ),
                            ]
                            .into_iter()
                            .map(|(name, key, icon, command)| {
                                let view = view.clone();
                                let menu_view = menu_view.clone();
                                crate::styles::button(
                                    gpui::SharedString::from(format!(
                                        "application-menu-layout-{name}"
                                    )),
                                    cx,
                                )
                                .debug_selector(move || format!("application-menu-layout-{name}"))
                                .ghost()
                                .small()
                                .icon(icon)
                                .tooltip(i18n::text(&language, key))
                                .selected(layout == name)
                                .flex_1()
                                .on_click(move |_, window, cx| {
                                    view.update(cx, |this, cx| this.run_menu(command, window, cx))
                                        .ok();
                                    menu_view.update(cx, |_, cx| cx.notify()).ok();
                                    cx.stop_propagation();
                                })
                            }),
                        )
                    }));
                }
                menu
            })
            .when(beside, |menu| menu.side(if crate::appearance::rtl(cx) { gpui::Corner::TopRight } else { gpui::Corner::TopLeft }, gpui::px(10. * self.state.settings.zoom), gpui::px(5. * self.state.settings.zoom)))
            .into_any_element()
    }

    pub fn sync_app_menu(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !cfg!(target_os = "macos") || cfg!(test) {
            return;
        }
        let input = window.context_stack().iter().any(|context| {
            context.contains("Input")
                || context.contains("NumberInput")
                || context.contains("RecordingShortcut")
        });
        let signature = Signature::new(
            &self.state.settings,
            self.playing,
            self.state.queue.shuffle,
            self.state.queue.repeat != Repeat::Off,
            input,
        );
        if self.app_menu.as_ref() == Some(&signature) {
            return;
        }
        cx.set_menus(signature.menus());
        #[cfg(target_os = "macos")]
        update_checkmarks(&signature);
        self.app_menu = Some(signature);
    }
    pub fn run_menu(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        use Command::*;
        match command {
            Settings | Servers => {
                self.close_command_palette(window, cx);
                self.navigate(
                    if command == Settings {
                        Route::Settings
                    } else {
                        Route::Servers
                    },
                    cx,
                );
            }
            CreatePlaylist => {
                self.close_command_palette(window, cx);
                self.edit_playlist(None, vec![], window, cx);
            }
            Private | PrivateOn | PrivateOff => {
                self.state.settings.private_mode = match command {
                    PrivateOn => true,
                    PrivateOff => false,
                    _ => !self.state.settings.private_mode,
                };
                self.save();
                self.update_presence(cx);
                cx.notify();
            }
            Sidebar | ExpandSidebar | CollapseSidebar => {
                self.state.settings.sidebar.collapsed = match command {
                    ExpandSidebar => false,
                    CollapseSidebar => true,
                    _ => !self.state.settings.sidebar.collapsed,
                };
                self.save();
                cx.notify();
            }
            Show => self.tray_command(crate::tray::Command::Show, window, cx),
            Close => self.close_native_window(window, cx),
            Hide => cx.hide(),
            HideOthers => cx.hide_other_apps(),
            ShowAll => cx.unhide_other_apps(),
            Minimize => window.minimize_window(),
            ReleaseNotes => {
                self.close_command_palette(window, cx);
                self.open_release_notes(window, cx);
            }
            About => {
                #[cfg(target_os = "macos")]
                if !cfg!(test)
                    && let Some(marker) = objc2::MainThreadMarker::new()
                {
                    objc2_app_kit::NSApplication::sharedApplication(marker)
                        .orderFrontStandardAboutPanel(None);
                }
            }
            Learn => cx.open_url("https://github.com/jeffvli/feishin"),
            Documentation => {
                cx.open_url("https://github.com/jeffvli/feishin?tab=readme-ov-file#getting-started")
            }
            Community => cx.open_url("https://github.com/jeffvli/feishin/discussions"),
            Issues => cx.open_url("https://github.com/jeffvli/feishin/issues"),
            Reload => {
                self.apply_theme(window, cx);
                if self.route == Route::NoNetwork {
                    self.retry_server_connection(cx);
                } else {
                    self.reload(false, cx);
                }
                window.refresh();
            }
            Restart => self.restart_native(window, cx),
            Palette => self.open_command_palette(window, cx),
            Back => self.back(cx),
            Forward => self.forward(cx),
            Inspector => window.dispatch_action(Box::new(gpui_component::ToggleInspector), cx),
            PanelHorizontal | PanelVertical => {
                self.state.settings.panels.layout = if command == PanelHorizontal {
                    "horizontal"
                } else {
                    "vertical"
                }
                .into();
                self.save();
                cx.notify();
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn update_checkmarks(signature: &Signature) {
    let Some(marker) = objc2::MainThreadMarker::new() else {
        return;
    };
    let Some(menu) = objc2_app_kit::NSApplication::sharedApplication(marker).mainMenu() else {
        return;
    };
    for section in menu.itemArray() {
        if let Some(submenu) = section.submenu() {
            for item in submenu.itemArray() {
                item.setState(if signature.checked(&item.title().to_string()) {
                    1
                } else {
                    0
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, RunMenu, Signature, Unavailable, typing_sensitive};
    use crate::{
        app::{Feishin, Route},
        shortcuts::RunShortcut,
    };
    use feishin_core::{
        shortcuts::{Action, Binding},
        storage::{self, Settings, State},
    };
    use gpui::{AppContext, MenuItem, TestAppContext};
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn original_menu_commands_edit_roles_playback_focus_and_toggle_states_are_preserved() {
        let mut settings = Settings {
            private_mode: true,
            ..Default::default()
        };
        settings.sidebar.collapsed = true;
        settings.shortcuts.insert(
            Action::Next,
            Binding {
                key: "n".into(),
                global: false,
                ..Binding::default()
            },
        );
        settings.shortcuts.insert(
            Action::Previous,
            Binding {
                key: "alt-left".into(),
                global: false,
                ..Binding::default()
            },
        );
        let state = Signature::new(&settings, true, true, true, true);
        let menus = state.menus();
        assert_eq!(
            menus
                .iter()
                .map(|menu| menu.name.as_ref())
                .collect::<Vec<_>>(),
            [
                "Feishin", "File", "Edit", "View", "Playback", "Window", "Help"
            ]
        );
        let MenuItem::Action { name, action, .. } = &menus[4].items[0] else {
            panic!()
        };
        assert_eq!(name.as_ref(), "Pause");
        assert!(action.as_any().is::<Unavailable>());
        let MenuItem::Action { action, .. } = &menus[4].items[2] else {
            panic!()
        };
        assert!(action.as_any().is::<Unavailable>());
        let MenuItem::Action { action, .. } = &menus[4].items[3] else {
            panic!()
        };
        assert_eq!(
            action
                .as_any()
                .downcast_ref::<RunShortcut>()
                .unwrap()
                .action,
            Action::Previous
        );
        assert!(
            menus[2]
                .items
                .iter()
                .filter(|item| matches!(
                    item,
                    MenuItem::Action {
                        os_action: Some(_),
                        ..
                    }
                ))
                .count()
                == 6
        );
        for name in ["Private Session", "Collapse Sidebar", "Shuffle", "Repeat"] {
            assert!(state.checked(&state.text(name)), "{name}");
        }
        let stopped = Signature::new(&Settings::default(), false, false, false, false);
        assert!(!stopped.checked("Shuffle"));
        let MenuItem::Action { name, action, .. } = &stopped.menus()[4].items[0] else {
            panic!()
        };
        assert_eq!(name.as_ref(), "Play");
        assert_eq!(
            action
                .as_any()
                .downcast_ref::<RunShortcut>()
                .unwrap()
                .action,
            Action::PlayPause
        );
        for key in ["space", "n", "shift-n", "enter"] {
            assert!(typing_sensitive(key), "{key}");
        }
        for key in ["", "cmd-k", "alt-left", "ctrl-p", "f1", "f24"] {
            assert!(!typing_sensitive(key), "{key}");
        }
    }

    fn click_selector(cx: &mut gpui::VisualTestContext, selector: &str) {
        let position = cx.debug_bounds(selector).unwrap().center();
        cx.simulate_click(position, gpui::Modifiers::none());
        cx.run_until_parked();
    }

    fn accessible_menu_labels(tree: &gpui::accesskit::TreeUpdate) -> Vec<String> {
        use gpui::{
            Role,
            accesskit::{Node, NodeId},
        };
        let nodes = tree
            .nodes
            .iter()
            .map(|(id, node)| (*id, node))
            .collect::<std::collections::HashMap<_, _>>();
        fn text(node: &Node, nodes: &std::collections::HashMap<NodeId, &Node>) -> String {
            if node.role() == Role::Label {
                return node
                    .value()
                    .or_else(|| node.label())
                    .unwrap_or_default()
                    .to_owned();
            }
            node.children()
                .iter()
                .filter_map(|id| nodes.get(id))
                .map(|node| text(node, nodes))
                .collect::<Vec<_>>()
                .join("")
        }
        fn contains_button(node: &Node, nodes: &std::collections::HashMap<NodeId, &Node>) -> bool {
            node.role() == Role::Button
                || node
                    .children()
                    .iter()
                    .filter_map(|id| nodes.get(id))
                    .any(|node| contains_button(node, nodes))
        }
        tree.nodes
            .iter()
            .filter(|(_, node)| node.role() == Role::MenuItem && !contains_button(node, &nodes))
            .map(|(_, node)| text(node, &nodes))
            .collect()
    }

    #[gpui::test]
    fn application_menu_matches_original_preferences_and_locales(cx: &mut TestAppContext) {
        use feishin_core::{
            i18n,
            model::{Server, ServerType},
        };
        use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, prelude::*};
        struct Probe {
            view: gpui::Entity<Feishin>,
            _subscription: gpui::Subscription,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                gpui::div().p_4().child(
                    self.view
                        .update(cx, |view, cx| view.application_menu_button(None, cx)),
                )
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory = std::env::temp_dir().join(format!(
            "feishin-application-menu-reference-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let view =
                cx.new(|cx| Feishin::new(directory.clone(), Ok(State::default()), window, cx));
            captured.replace(Some(view.clone()));
            let probe = cx.new(|cx| Probe {
                view: view.clone(),
                _subscription: cx.observe(&view, |_, _, cx| cx.notify()),
            });
            Root::new(probe, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../assets/application-menu-reference.json"))
                .unwrap();
        for language in ["en", "fr", "ar"] {
            for case in fixture
                .as_array()
                .unwrap()
                .iter()
                .filter(|case| case["electron"] == true)
            {
                cx.update(|_, cx| {
                    view.update(cx, |view, cx| {
                        view.state.settings.language = language.into();
                        view.state.settings.sidebar.collapsed =
                            case["collapsed"].as_bool().unwrap();
                        view.state.settings.private_mode = case["privateMode"].as_bool().unwrap();
                        view.state.settings.panels.kind = case["kind"].as_str().unwrap().into();
                        view.state.settings.panels.layout = case["layout"].as_str().unwrap().into();
                        let server = Server {
                            id: uuid::Uuid::new_v4(),
                            name: "Fixture server".into(),
                            username: "fixture".into(),
                            url: "http://127.0.0.1:1".into(),
                            kind: ServerType::Navidrome,
                            legacy_auth: false,
                            options: Default::default(),
                        };
                        view.state.active_server =
                            case["server"].as_bool().unwrap().then_some(server.id);
                        view.state.servers = vec![server];
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                click_selector(cx, "application-menu");
                cx.run_until_parked();
                let tree =
                    cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
                let mut actual = accessible_menu_labels(&tree);
                if case["server"] == true {
                    assert_eq!(actual.remove(0), "Fixture server");
                }
                let expected = case["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|item| item["type"] == "item")
                    .map(|item| {
                        let label = &item["label"];
                        i18n::translate_with(
                            language,
                            label["key"].as_str().unwrap(),
                            &label["options"],
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "{language}: {case}");
                for (name, key) in [
                    ("horizontal", "setting.sidePlayQueueLayout_optionHorizontal"),
                    ("vertical", "setting.sidePlayQueueLayout_optionVertical"),
                ] {
                    let label = i18n::translate(language, key).unwrap();
                    let button = tree.nodes.iter().find(|(_, node)| {
                        node.role() == gpui::Role::Button && node.label() == Some(label.as_str())
                    });
                    if case["kind"] == "sideQueue" {
                        assert_eq!(
                            button.unwrap().1.is_selected(),
                            Some(case["layout"] == name)
                        );
                    } else {
                        assert!(button.is_none());
                    }
                }
                cx.simulate_keystrokes("escape");
                cx.run_until_parked();
            }
        }
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn application_menu_mouse_actions_preserve_focus_navigation_and_saved_layout(
        cx: &mut TestAppContext,
    ) {
        use gpui::{Modifiers, Role, point, px};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory = std::env::temp_dir().join(format!(
            "feishin-application-menu-actions-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let view =
                cx.new(|cx| Feishin::new(directory.clone(), Ok(State::default()), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        fn choose(cx: &mut gpui::VisualTestContext, key: &str) {
            use gpui::accesskit::{Action, ActionRequest, TreeId};
            cx.update(|window, cx| {
                let tree = window.accessibility_tree_for_test().unwrap();
                let (id, node) = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| node.role() == Role::Button && node.label() == Some("Menu"))
                    .unwrap();
                assert!(
                    node.supports_action(Action::Click),
                    "Menu must expose activation"
                );
                assert_eq!(node.is_expanded(), Some(false));
                window.dispatch_accessibility_action_for_test(
                    ActionRequest {
                        action: if key == "page.appMenu.settings" {
                            Action::Focus
                        } else {
                            Action::Click
                        },
                        target_tree: TreeId::ROOT,
                        target_node: *id,
                        data: None,
                    },
                    cx,
                );
            });
            cx.run_until_parked();
            if key == "page.appMenu.settings" {
                cx.simulate_keystrokes("enter");
                cx.simulate_event(gpui::KeyUpEvent {
                    keystroke: gpui::Keystroke::parse("enter").unwrap(),
                });
                cx.run_until_parked();
            }
            let label = feishin_core::i18n::translate("en", key).unwrap();
            let bounds = cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                assert!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.label() == Some("Menu")
                            && node.is_expanded() == Some(true)),
                    "Menu did not open for {key}"
                );
                let bounds = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| {
                        node.role() == Role::MenuItem
                            && node.children().iter().any(|id| {
                                tree.nodes.iter().any(|(child_id, child)| {
                                    child_id == id
                                        && child.role() == Role::Label
                                        && child.value() == Some(label.as_str())
                                })
                            })
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "Menu item missing: {label}, labels {:?}",
                            accessible_menu_labels(tree)
                        )
                    })
                    .1
                    .bounds()
                    .unwrap();
                let scale = f64::from(window.scale_factor());
                point(
                    px(((bounds.x0 + bounds.x1) / 2. / scale) as f32),
                    px(((bounds.y0 + bounds.y1) / 2. / scale) as f32),
                )
            });
            cx.simulate_click(bounds, Modifiers::none());
            cx.run_until_parked();
        }
        choose(cx, "page.appMenu.commandPalette");
        assert!(view.read_with(cx, |view, _| view.command_palette.is_some()));
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        assert_eq!(
            tree.nodes
                .iter()
                .find(|(id, _)| *id == tree.focus)
                .unwrap()
                .1
                .role(),
            Role::TextInput
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(310));
        cx.refresh().unwrap();
        cx.run_until_parked();
        choose(cx, "page.appMenu.settings");
        assert_eq!(
            view.read_with(cx, |view, _| view.route.clone()),
            Route::Settings
        );
        choose(cx, "page.appMenu.privateModeOn");
        assert!(view.read_with(cx, |view, _| view.state.settings.private_mode));
        choose(cx, "page.appMenu.privateModeOff");
        assert!(!view.read_with(cx, |view, _| view.state.settings.private_mode));
        choose(cx, "page.appMenu.collapseSidebar");
        assert!(view.read_with(cx, |view, _| view.state.settings.sidebar.collapsed));
        choose(cx, "page.appMenu.goBack");
        assert_eq!(
            view.read_with(cx, |view, _| view.route.clone()),
            Route::Servers
        );
        choose(cx, "page.appMenu.goForward");
        assert_eq!(
            view.read_with(cx, |view, _| view.route.clone()),
            Route::Settings
        );
        choose(cx, "page.appMenu.expandSidebar");
        assert!(!view.read_with(cx, |view, _| view.state.settings.sidebar.collapsed));
        click_selector(cx, "application-menu");
        cx.run_until_parked();
        for name in ["vertical", "horizontal"] {
            click_selector(cx, &format!("application-menu-layout-{name}"));
            cx.run_until_parked();
            assert_eq!(
                view.read_with(cx, |view, _| view.state.settings.panels.layout.clone()),
                name
            );
            let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
            assert!(
                tree.nodes.iter().any(|(_, node)| node.role() == Role::Menu),
                "Layout controls keep the menu open"
            );
            assert_eq!(
                storage::load(&directory.join("state.json"))
                    .unwrap()
                    .settings
                    .panels
                    .layout,
                name
            );
        }
        cx.simulate_keystrokes("escape");
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.full_player = true;
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(600));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        choose(cx, "page.appMenu.privateModeOn");
        assert!(view.read_with(cx, |view, _| view.state.settings.private_mode));
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.full_player = false;
                cx.notify();
            })
        });
        cx.run_until_parked();
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn menu_library_selection_matches_original_single_and_multiple_scopes(cx: &mut TestAppContext) {
        use feishin_core::{
            api::Api,
            model::{Server, ServerType, Session},
        };
        cx.update(gpui_component::init);
        let directory = std::env::temp_dir().join(format!(
            "feishin-menu-library-selection-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view =
                cx.new(|cx| Feishin::new(directory.clone(), Ok(State::default()), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/application-menu-library-reference.json"
        ))
        .unwrap();
        for case in fixture.as_array().unwrap() {
            for kind in ServerType::ALL
                .into_iter()
                .filter(|kind| (*kind == ServerType::Navidrome) == case["multi"].as_bool().unwrap())
            {
                cx.update(|_, cx| {
                    view.update(cx, |view, cx| {
                        let selected = case["selected"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|id| id.as_str().unwrap().to_owned())
                            .collect();
                        let server = Server {
                            id: uuid::Uuid::new_v4(),
                            name: "Fixture".into(),
                            username: "fixture".into(),
                            url: "http://127.0.0.1:1".into(),
                            kind,
                            legacy_auth: false,
                            options: feishin_core::model::ServerOptions {
                                music_folder_ids: selected,
                                ..Default::default()
                            },
                        };
                        let id = server.id;
                        view.state.servers = vec![server.clone()];
                        view.state.active_server = Some(id);
                        view.sessions.clear();
                        view.sessions
                            .insert(id, Api::new(server, Session::default()).unwrap());
                        view.route = Route::Servers;
                        view.state.queue.add(
                            vec![feishin_core::model::Item {
                                id: "preserve".into(),
                                server_id: id,
                                ..Default::default()
                            }],
                            feishin_core::queue::Enqueue::Now,
                        );
                        let key = view.state.queue.current;
                        view.select_menu_library(id, case["folder"].as_str(), cx);
                        let expected = case["expected"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|id| id.as_str().unwrap().to_owned())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            view.state.servers[0].options.music_folder_ids, expected,
                            "{kind:?} {case}"
                        );
                        assert_eq!(view.sessions[&id].server.options.music_folder_ids, expected);
                        assert_eq!(view.state.queue.current, key);
                        let saved = storage::load(&directory.join("state.json")).unwrap();
                        assert_eq!(saved.servers[0].options.music_folder_ids, expected);
                        view.state.active_server = None;
                        view.select_menu_library(id, Some("stale-account"), cx);
                        assert_eq!(view.state.servers[0].options.music_folder_ids, expected);
                    })
                });
            }
        }
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn account_submenu_loads_library_rows_and_recovers_expired_accounts(cx: &mut TestAppContext) {
        use feishin_core::{
            api::Api,
            model::{Server, ServerType, Session},
        };
        use gpui::{Modifiers, Role, point, px};
        use std::{
            io::{BufRead, BufReader, Write},
            net::TcpListener,
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            thread,
            time::Duration,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let body = if line.starts_with("GET /api/playlist?") {
                    "[]"
                } else {
                    assert!(line.contains("getMusicFolders"), "{line}");
                    r#"{"subsonic-response":{"status":"ok","musicFolders":{"musicFolder":[{"id":1,"name":"First library"},{"id":2,"name":"Second library"}]}}}"#
                };
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            }
        });
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory = std::env::temp_dir().join(format!(
            "feishin-menu-library-rows-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let id = uuid::Uuid::new_v4();
        let expired = uuid::Uuid::new_v4();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let view = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            view.update(cx, |view, cx| {
                let server = Server {
                    id,
                    name: "Active account".into(),
                    username: "fixture".into(),
                    url: format!("http://{address}"),
                    kind: ServerType::Navidrome,
                    legacy_auth: false,
                    options: Default::default(),
                };
                view.state.servers = vec![
                    server.clone(),
                    Server {
                        id: expired,
                        name: "Expired account".into(),
                        ..server.clone()
                    },
                ];
                view.state.active_server = Some(id);
                view.sessions
                    .insert(id, Api::new(server, Session::default()).unwrap());
                cx.notify();
            });
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        fn open_account(cx: &mut gpui::VisualTestContext) {
            click_selector(cx, "application-menu");
            cx.simulate_keystrokes("down");
            cx.simulate_keystrokes("right");
            cx.run_until_parked();
        }
        fn choose(cx: &mut gpui::VisualTestContext, label: &str) {
            let position = cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                let bounds = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| {
                        node.role() == Role::MenuItem
                            && node.children().iter().any(|id| {
                                tree.nodes.iter().any(|(child_id, child)| {
                                    child_id == id
                                        && child.role() == Role::Label
                                        && child.value() == Some(label)
                                })
                            })
                    })
                    .unwrap_or_else(|| {
                        panic!("Missing {label}: {:?}", accessible_menu_labels(tree))
                    })
                    .1
                    .bounds()
                    .unwrap();
                let scale = f64::from(window.scale_factor());
                point(
                    px(((bounds.x0 + bounds.x1) / 2. / scale) as f32),
                    px(((bounds.y0 + bounds.y1) / 2. / scale) as f32),
                )
            });
            cx.simulate_click(position, Modifiers::none());
            cx.run_until_parked();
        }
        open_account(cx);
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        assert!(accessible_menu_labels(&tree).contains(&"First library".into()));
        assert!(accessible_menu_labels(&tree).contains(&"Second library".into()));
        choose(cx, "First library");
        assert_eq!(
            view.read_with(cx, |view, _| view.sessions[&id]
                .server
                .options
                .music_folder_ids
                .clone()),
            ["1"]
        );
        open_account(cx);
        choose(
            cx,
            &feishin_core::i18n::translate("en", "page.appMenu.manageServers").unwrap(),
        );
        assert_eq!(
            view.read_with(cx, |view, _| view.route.clone()),
            Route::Servers
        );
        open_account(cx);
        choose(cx, "Expired account");
        assert_eq!(
            view.read_with(cx, |view, _| view.editing_server),
            Some(expired)
        );
        assert_eq!(
            view.read_with(cx, |view, _| view.state.active_server),
            Some(id)
        );
        stop.store(true, Ordering::SeqCst);
        worker.join().unwrap();
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn app_menu_settings_navigation_private_sidebar_and_playlist_commands_use_live_actions(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-app-menu-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            let view = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        cx.simulate_keystrokes("secondary-,");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |view, _| view.route.clone()),
            Route::Settings
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                for command in [Command::Private, Command::Sidebar] {
                    view.run_menu(command, window, cx);
                }
            })
        });
        let saved = storage::load(&directory.join("state.json")).unwrap();
        assert!(saved.settings.private_mode);
        assert!(saved.settings.sidebar.collapsed);
        cx.dispatch_action(RunMenu {
            command: Command::Servers,
        });
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |view, _| view.route.clone()),
            Route::Servers
        );
        cx.dispatch_action(RunMenu {
            command: Command::CreatePlaylist,
        });
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |view, _| view.route.clone()),
            Route::PlaylistEditor
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
