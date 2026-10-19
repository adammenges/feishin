use crate::app::Feishin;
#[cfg(target_os = "macos")]
use anyhow::Context as _;
use anyhow::{Result, bail};
use feishin_core::{i18n, shortcuts::Action};
use gpui::{Context, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Playback(Action),
    Show,
    ToggleWindow,
    Quit,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, gpui::Action)]
#[action(namespace = feishin, no_json)]
pub struct DockPlayback {
    pub action: Action,
}

fn dock_menu(language: &str) -> Vec<gpui::MenuItem> {
    [
        ("Play/Pause", Action::PlayPause),
        ("Next Track", Action::Next),
        ("Previous Track", Action::Previous),
        ("Stop", Action::Stop),
    ]
    .into_iter()
    .map(|(label, action)| {
        gpui::MenuItem::action(i18n::text(language, label), DockPlayback { action })
    })
    .collect()
}

fn menu_command(id: &str) -> Option<Command> {
    Some(match id.strip_prefix("feishin-tray-")? {
        "play" => Command::Playback(Action::PlayPause),
        "next" => Command::Playback(Action::Next),
        "previous" => Command::Playback(Action::Previous),
        "stop" => Command::Playback(Action::Stop),
        "show" => Command::Show,
        "quit" => Command::Quit,
        _ => return None,
    })
}

pub struct Tray {
    _icon: TrayIcon,
    items: Vec<(MenuItem, &'static str)>,
    language: String,
}
impl Tray {
    fn new(language: &str) -> Result<Self> {
        let png: &[u8] = if cfg!(target_os = "macos") {
            include_bytes!("../../../assets/icons/IconTemplate.png")
        } else {
            include_bytes!("../../../assets/icons/icon.png")
        };
        let image = image::load_from_memory(png)?.into_rgba8();
        let icon = Icon::from_rgba(image.as_raw().clone(), image.width(), image.height())?;
        let menu = Menu::new();
        let mut items = Vec::new();
        for (id, label) in [
            ("play", "Play/Pause"),
            ("next", "Next Track"),
            ("previous", "Previous Track"),
            ("stop", "Stop"),
            ("show", "Open main window"),
            ("quit", "Quit"),
        ] {
            if id == "show" {
                menu.append(&PredefinedMenuItem::separator())?;
            }
            let item = MenuItem::with_id(
                format!("feishin-tray-{id}"),
                i18n::text(language, label),
                true,
                None,
            );
            menu.append(&item)?;
            items.push((item, label));
        }
        let builder = TrayIconBuilder::new()
            .with_id("feishin-native")
            .with_tooltip("Feishin")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(cfg!(target_os = "macos"));
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        let icon = builder.build()?;
        Ok(Self {
            _icon: icon,
            items,
            language: language.to_owned(),
        })
    }
    fn update_language(&mut self, language: &str) {
        if self.language != language {
            for (item, label) in &self.items {
                item.set_text(i18n::text(language, label));
            }
            self.language = language.to_owned();
        }
    }
    fn poll(&self) -> Vec<Command> {
        let mut commands: Vec<_> = MenuEvent::receiver()
            .try_iter()
            .filter_map(|event| menu_command(event.id.as_ref()))
            .collect();
        for event in TrayIconEvent::receiver().try_iter() {
            if !cfg!(target_os = "macos")
                && matches!(event, TrayIconEvent::Click {id, button:tray_icon::MouseButton::Left, button_state:tray_icon::MouseButtonState::Up, ..} if id.as_ref() == "feishin-native")
            {
                commands.push(Command::ToggleWindow);
            }
        }
        commands
    }
}

impl Feishin {
    pub fn init_dock(&self, window: &Window, cx: &mut Context<Self>) {
        let entity = cx.entity().downgrade();
        let handle = window.window_handle();
        gpui::App::on_action(cx, move |action: &DockPlayback, cx| {
            let entity = entity.clone();
            let action = action.action;
            // Menu actions can arrive during a window update. Wait until that borrow ends.
            cx.defer(move |cx| {
                handle
                    .update(cx, |_, window, cx| {
                        entity
                            .update(cx, |this, cx| this.run_shortcut(action, window, cx))
                            .ok();
                    })
                    .ok();
            });
        });
    }

    pub fn sync_dock(&mut self, cx: &mut Context<Self>) {
        if cfg!(target_os = "macos") && self.dock_language != self.state.settings.language {
            self.dock_language = self.state.settings.language.clone();
            if !cfg!(test) {
                cx.set_dock_menu(dock_menu(&self.dock_language));
            }
        }
    }

    pub fn sync_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // GPUI's test windows have no native handle. Interactive OS checks are separate.
        if cfg!(test) {
            return;
        }
        self.native_hide_supported = HasWindowHandle::window_handle(window).is_ok_and(|handle| {
            matches!(
                handle.as_raw(),
                RawWindowHandle::AppKit(_)
                    | RawWindowHandle::Win32(_)
                    | RawWindowHandle::Xlib(_)
                    | RawWindowHandle::Xcb(_)
                    | RawWindowHandle::Wayland(_)
            )
        });
        if !self.state.settings.tray {
            if self.window_hidden {
                self.show_native_window(window, cx);
            }
            self.tray = None;
            if !self.tray_initialized {
                self.tray_initialized = true;
                if self.state.settings.start_minimized {
                    window.minimize_window();
                }
            }
            return;
        }
        if self.tray.is_none() && self.tray_error.is_none() {
            match Tray::new(&self.state.settings.language) {
                Ok(tray) => self.tray = Some(tray),
                Err(error) => {
                    log::warn!("Cannot create tray icon: {error}");
                    self.tray_error = Some(error.to_string());
                    self.message = format!("Cannot create tray icon: {error}");
                    cx.notify();
                }
            }
        }
        if let Some(tray) = &mut self.tray {
            tray.update_language(&self.state.settings.language);
            let commands = tray.poll();
            for command in commands {
                self.tray_command(command, window, cx);
            }
        }
        if !self.tray_initialized {
            self.tray_initialized = true;
            if self.state.settings.start_minimized {
                if self.state.settings.minimize_to_tray
                    && self.tray.is_some()
                    && self.native_hide_supported
                {
                    self.hide_native_window(window, cx);
                } else {
                    window.minimize_window();
                }
            }
        }
        if self.state.settings.minimize_to_tray
            && self.tray.is_some()
            && self.native_hide_supported
            && !self.window_hidden
            && minimized(window).unwrap_or(false)
        {
            self.hide_native_window(window, cx);
        }
    }
    pub fn tray_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::Playback(action) => self.run_shortcut(action, window, cx),
            Command::Show => self.show_native_window(window, cx),
            Command::ToggleWindow => {
                if self.state.settings.minimize_to_tray && !self.window_hidden {
                    self.hide_native_window(window, cx);
                } else {
                    self.show_native_window(window, cx);
                }
            }
            Command::Quit => {
                self.remember_window(window);
                self.save();
                cx.quit();
            }
        }
    }
    pub fn hide_native_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match visible(window, false) {
            Ok(()) => {
                self.window_hidden = true;
                self.remember_window(window);
                self.save();
            }
            Err(error) => {
                log::warn!("Cannot hide native window: {error}");
                self.message = error.to_string();
                cx.notify();
            }
        }
    }
    fn show_native_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !cfg!(test) && (self.native_hide_supported || cfg!(target_os = "macos") || cfg!(windows))
        {
            match visible(window, true) {
                Ok(()) => self.window_hidden = false,
                Err(error) => {
                    log::warn!("Cannot restore native window: {error}");
                    self.message = error.to_string();
                    cx.notify();
                }
            }
        }
        window.activate_window();
        cx.activate(true);
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn mac_window(window: &Window) -> Result<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window)?.as_raw() else {
        bail!("Expected an AppKit window")
    };
    // GPUI owns the NSView and invokes this on its main UI thread.
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
    view.window().context("The native view has no window")
}

fn visible(window: &Window, show: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let window = mac_window(window)?;
        if show {
            window.deminiaturize(None);
            window.makeKeyAndOrderFront(None);
        } else {
            window.deminiaturize(None);
            window.orderOut(None);
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{IsIconic, SW_HIDE, SW_RESTORE, SW_SHOW, ShowWindow},
        };
        let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window)?.as_raw()
        else {
            bail!("Expected a Win32 window")
        };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        // GPUI owns the HWND on this UI thread.
        unsafe {
            let _ = ShowWindow(
                hwnd,
                if !show {
                    SW_HIDE
                } else if IsIconic(hwnd).as_bool() {
                    SW_RESTORE
                } else {
                    SW_SHOW
                },
            );
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        if matches!(
            HasWindowHandle::window_handle(window)?.as_raw(),
            RawWindowHandle::Wayland(_)
        ) {
            anyhow::ensure!(
                window.set_visible(show),
                "GPUI cannot change this Wayland window's visibility"
            );
            return Ok(());
        }
        use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
        let id = x11_window(window)?;
        let (connection, _) = x11rb::connect(None)?;
        if show {
            connection.map_window(id)?.check()?;
        } else {
            connection.unmap_window(id)?.check()?;
        }
        connection.flush()?;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn x11_window(window: &Window) -> Result<u32> {
    match HasWindowHandle::window_handle(window)?.as_raw() {
        RawWindowHandle::Xcb(handle) => Ok(handle.window.get()),
        RawWindowHandle::Xlib(handle) => Ok(u32::try_from(handle.window)?),
        _ => bail!("Expected an X11 window"),
    }
}

fn minimized(window: &Window) -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        Ok(mac_window(window)?.isMiniaturized())
    }
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsIconic};
        let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window)?.as_raw()
        else {
            bail!("Expected a Win32 window")
        };
        Ok(unsafe { IsIconic(HWND(handle.hwnd.get() as *mut _)).as_bool() })
    }
    #[cfg(target_os = "linux")]
    {
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
        let id = x11_window(window)?;
        let (connection, _) = x11rb::connect(None)?;
        let atom = connection.intern_atom(false, b"WM_STATE")?.reply()?.atom;
        let property = connection
            .get_property(false, id, atom, AtomEnum::ANY, 0, 1)?
            .reply()?;
        Ok(property.value32().and_then(|mut values| values.next()) == Some(3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tray_menu_commands_reuse_validated_transport_actions() {
        assert_eq!(
            menu_command("feishin-tray-next"),
            Some(Command::Playback(Action::Next))
        );
        assert_eq!(menu_command("feishin-tray-show"), Some(Command::Show));
        assert_eq!(menu_command("feishin-tray-quit"), Some(Command::Quit));
        assert_eq!(menu_command("feishin-tray-arbitrary"), None);
        assert_eq!(menu_command("next"), None);
        let actions: Vec<_> = dock_menu("en")
            .into_iter()
            .map(|item| {
                let gpui::MenuItem::Action { action, .. } = item else {
                    panic!("Expected transport action")
                };
                action
                    .as_any()
                    .downcast_ref::<DockPlayback>()
                    .unwrap()
                    .action
            })
            .collect();
        assert_eq!(
            actions,
            [
                Action::PlayPause,
                Action::Next,
                Action::Previous,
                Action::Stop
            ]
        );
    }
}
