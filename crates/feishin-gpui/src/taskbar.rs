#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Previous,
    PlayPause,
    Next,
}
#[cfg(any(windows, test))]
fn command(wparam: usize) -> Option<Command> {
    if (wparam >> 16) & 0xffff != 0x1800 {
        return None;
    }
    match wparam & 0xffff {
        1001 => Some(Command::Previous),
        1002 => Some(Command::PlayPause),
        1003 => Some(Command::Next),
        _ => None,
    }
}
#[cfg(any(windows, test))]
fn tooltip(text: &str) -> [u16; 260] {
    let mut result = [0; 260];
    let mut units: Vec<_> = text.encode_utf16().take(259).collect();
    if units
        .last()
        .is_some_and(|unit| (0xd800..=0xdbff).contains(unit))
    {
        units.pop();
    }
    result[..units.len()].copy_from_slice(&units);
    result
}
#[cfg(any(windows, test))]
fn icon_resource(png: &[u8]) -> anyhow::Result<Vec<u8>> {
    let pixels = image::load_from_memory(png)?
        .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    // Win32 icon resource: BITMAPINFOHEADER, bottom-up BGRA pixels, then 1-bit AND mask.
    let mut result = Vec::with_capacity(40 + 4096 + 128);
    for number in [40_u32, 32, 64] {
        result.extend(number.to_le_bytes());
    }
    result.extend(1_u16.to_le_bytes());
    result.extend(32_u16.to_le_bytes());
    for number in [0_u32, 4096, 0, 0, 0, 0] {
        result.extend(number.to_le_bytes());
    }
    let mut mask = [0_u8; 128];
    for y in (0..32).rev() {
        for x in 0..32 {
            let pixel = pixels.get_pixel(x, y).0;
            result.extend([pixel[2], pixel[1], pixel[0], pixel[3]]);
            if pixel[3] == 0 {
                mask[((31 - y) * 4 + x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
    }
    result.extend(mask);
    Ok(result)
}

#[cfg(windows)]
pub use native::Taskbar;
#[cfg(windows)]
pub fn configure_app_id() -> anyhow::Result<()> {
    // Match the original application so Windows groups its windows and media controls.
    unsafe {
        windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(windows::core::w!(
            "org.jeffvli.feishin"
        ))?;
    }
    Ok(())
}
#[cfg(windows)]
mod native {
    use super::*;
    use anyhow::{Context, Result, ensure};
    use std::{
        cell::RefCell,
        sync::mpsc::{self, Receiver, SyncSender},
        time::{Duration, Instant},
    };
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, WPARAM},
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            UI::{
                Shell::{
                    DefSubclassProc, ITaskbarList3, RemoveWindowSubclass, SetWindowSubclass,
                    THB_FLAGS, THB_ICON, THB_TOOLTIP, THBF_ENABLED, THBF_HIDDEN, THUMBBUTTON,
                    TaskbarList,
                },
                WindowsAndMessaging::{
                    CreateIconFromResourceEx, DestroyIcon, HICON, IsWindow, LR_DEFAULTCOLOR,
                    RegisterWindowMessageW, WM_COMMAND, WM_NCDESTROY,
                },
            },
        },
        core::w,
    };
    const SUBCLASS: usize = 0x46454953;
    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }
    struct State {
        taskbar: ITaskbarList3,
        _apartment: Apartment,
        icons: Vec<HICON>,
        installed: bool,
        playing: bool,
        labels: [String; 3],
        attempted: Instant,
    }
    struct Callback {
        state: RefCell<State>,
        events: SyncSender<Command>,
        created: u32,
    }
    impl State {
        fn buttons(&self, hidden: bool) -> [THUMBBUTTON; 3] {
            std::array::from_fn(|index| THUMBBUTTON {
                dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                iId: 1001 + index as u32,
                hIcon: self.icons[if index == 1 && self.playing { 3 } else { index }],
                szTip: tooltip(&self.labels[index]),
                dwFlags: if hidden { THBF_HIDDEN } else { THBF_ENABLED },
                ..Default::default()
            })
        }
        fn install(&mut self, hwnd: HWND) {
            self.attempted = Instant::now();
            match unsafe { self.taskbar.ThumbBarAddButtons(hwnd, &self.buttons(false)) } {
                Ok(()) => self.installed = true,
                Err(error) => log::debug!("Windows taskbar is not ready: {error}"),
            }
        }
    }
    impl Drop for State {
        fn drop(&mut self) {
            for icon in &self.icons {
                let _ = unsafe { DestroyIcon(*icon) };
            }
        }
    }
    unsafe extern "system" fn subclass(
        hwnd: HWND,
        msg: u32,
        wp: WPARAM,
        lp: LPARAM,
        id: usize,
        data: usize,
    ) -> LRESULT {
        // GPUI owns this window on the same UI thread. The Box remains alive until removal.
        let context = unsafe { &*(data as *const Callback) };
        if msg == WM_COMMAND
            && let Some(command) = command(wp.0)
        {
            let _ = context.events.try_send(command);
            return LRESULT(0);
        }
        if msg == context.created
            && let Ok(mut state) = context.state.try_borrow_mut()
        {
            state.installed = false;
            state.install(hwnd);
        }
        if msg == WM_NCDESTROY {
            let _ = unsafe { RemoveWindowSubclass(hwnd, Some(subclass), id) };
        }
        unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
    }
    pub struct Taskbar {
        hwnd: HWND,
        context: Box<Callback>,
        events: Receiver<Command>,
    }
    impl Taskbar {
        pub fn new(handle: *mut std::ffi::c_void, labels: [String; 3]) -> Result<Self> {
            let hwnd = HWND(handle);
            ensure!(
                unsafe { IsWindow(Some(hwnd)) }.as_bool(),
                "Missing Windows taskbar window"
            );
            let apartment =
                Apartment(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
            let taskbar: ITaskbarList3 =
                unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) }?;
            unsafe { taskbar.HrInit() }?;
            let (events_tx, events) = mpsc::sync_channel(32);
            let mut state = State {
                taskbar,
                _apartment: apartment,
                icons: vec![],
                installed: false,
                playing: false,
                labels,
                attempted: Instant::now(),
            };
            let created = unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) };
            ensure!(created != 0, "Could not register Windows taskbar events");
            for png in [
                include_bytes!("../../../assets/skip-previous.png").as_slice(),
                include_bytes!("../../../assets/play-circle.png"),
                include_bytes!("../../../assets/skip-next.png"),
                include_bytes!("../../../assets/pause-circle.png"),
            ] {
                let resource = icon_resource(png)?;
                state.icons.push(
                    unsafe {
                        CreateIconFromResourceEx(&resource, true, 0x30000, 32, 32, LR_DEFAULTCOLOR)
                    }
                    .context("Could not create taskbar playback icon")?,
                );
            }
            let mut context = Box::new(Callback {
                state: RefCell::new(state),
                events: events_tx,
                created,
            });
            let data = (&mut *context as *mut Callback) as usize;
            ensure!(
                unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS, data) }.as_bool(),
                "Could not install Windows taskbar controls"
            );
            context.state.borrow_mut().install(hwnd);
            Ok(Self {
                hwnd,
                context,
                events,
            })
        }
        pub fn update(&mut self, playing: bool, labels: [String; 3]) {
            let mut state = self.context.state.borrow_mut();
            if !state.installed && state.attempted.elapsed() >= Duration::from_secs(1) {
                state.install(self.hwnd);
            }
            if playing != state.playing || labels != state.labels {
                state.playing = playing;
                state.labels = labels;
                if state.installed
                    && let Err(error) = unsafe {
                        state
                            .taskbar
                            .ThumbBarUpdateButtons(self.hwnd, &state.buttons(false))
                    }
                {
                    log::warn!("Windows taskbar button update failed: {error}");
                }
            }
        }
        pub fn poll(&self) -> Vec<Command> {
            self.events.try_iter().collect()
        }
    }
    impl Drop for Taskbar {
        fn drop(&mut self) {
            let state = self.context.state.borrow();
            if unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
                if state.installed {
                    let _ = unsafe {
                        state
                            .taskbar
                            .ThumbBarUpdateButtons(self.hwnd, &state.buttons(true))
                    };
                }
                let _ = unsafe { RemoveWindowSubclass(self.hwnd, Some(subclass), SUBCLASS) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toolbar_command_validation_utf16_tooltips_and_icon_resources() {
        assert_eq!(command((0x1800 << 16) | 1001), Some(Command::Previous));
        assert_eq!(command((0x1800 << 16) | 1002), Some(Command::PlayPause));
        assert_eq!(command((0x1800 << 16) | 1003), Some(Command::Next));
        assert!(command(1002).is_none() && command((0x1800 << 16) | 999).is_none());
        let tip = tooltip(&format!("{}😀", "x".repeat(258)));
        assert_eq!(tip[258], 0);
        assert_eq!(tip[259], 0);
        let icon = icon_resource(include_bytes!("../../../assets/play-circle.png")).unwrap();
        assert_eq!(icon.len(), 40 + 4096 + 128);
        assert_eq!(u32::from_le_bytes(icon[..4].try_into().unwrap()), 40);
        assert_eq!(u32::from_le_bytes(icon[8..12].try_into().unwrap()), 64);
        assert!(
            icon[40..4136]
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != 0)
        );
    }
}
