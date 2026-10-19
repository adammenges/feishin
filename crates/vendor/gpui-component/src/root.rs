use crate::{
    ActiveTheme, Placement,
    animation::CssTransition,
    dialog::Dialog,
    input::InputState,
    notification::{Notification, NotificationList},
    sheet::Sheet,
    window_border,
};
use gpui::{
    AnyView, App, AppContext, Context, DefiniteLength, Entity, FocusHandle, InteractiveElement,
    IntoElement, KeyBinding, ParentElement as _, Render, Styled, Subscription, Window, actions,
    canvas, div, prelude::FluentBuilder as _,
};
use std::{any::TypeId, cell::RefCell, rc::Rc, time::Duration};

actions!(root, [Tab, TabPrev]);

const CONTEXT: &str = "Root";
pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", Tab, Some(CONTEXT)),
        KeyBinding::new("shift-tab", TabPrev, Some(CONTEXT)),
    ]);
}

/// Extension trait for [`Window`] to add dialog, sheet .. functionality.
pub trait WindowExt: Sized {
    /// Opens a Sheet at right placement.
    fn open_sheet<F>(&mut self, cx: &mut App, build: F)
    where
        F: Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static;

    /// Opens a Sheet at the given placement.
    fn open_sheet_at<F>(&mut self, placement: Placement, cx: &mut App, build: F)
    where
        F: Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static;

    /// Return true, if there is an active Sheet.
    fn has_active_sheet(&mut self, cx: &mut App) -> bool;

    /// Closes the active Sheet.
    fn close_sheet(&mut self, cx: &mut App);

    /// Opens a Dialog.
    fn open_dialog<F>(&mut self, cx: &mut App, build: F)
    where
        F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static;

    /// Return true, if there is an active Dialog.
    fn has_active_dialog(&mut self, cx: &mut App) -> bool;

    /// Closes the last active Dialog.
    fn close_dialog(&mut self, cx: &mut App);

    /// Closes all active Dialogs.
    fn close_all_dialogs(&mut self, cx: &mut App);

    /// Pushes a notification to the notification list.
    fn push_notification(&mut self, note: impl Into<Notification>, cx: &mut App);

    /// Removes the notification with the given id.
    fn remove_notification<T: Sized + 'static>(&mut self, cx: &mut App);

    /// Clears all notifications.
    fn clear_notifications(&mut self, cx: &mut App);

    /// Returns number of notifications.
    fn notifications(&mut self, cx: &mut App) -> Rc<Vec<Entity<Notification>>>;

    /// Return current focused Input entity.
    fn focused_input(&mut self, cx: &mut App) -> Option<Entity<InputState>>;
    /// Returns true if there is a focused Input entity.
    fn has_focused_input(&mut self, cx: &mut App) -> bool;
}

impl WindowExt for Window {
    fn open_sheet<F>(&mut self, cx: &mut App, build: F)
    where
        F: Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static,
    {
        self.open_sheet_at(Placement::Right, cx, build)
    }

    fn open_sheet_at<F>(&mut self, placement: Placement, cx: &mut App, build: F)
    where
        F: Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static,
    {
        Root::update(self, cx, move |root, window, cx| {
            if root.active_sheet.is_none() {
                root.previous_focus_handle = window.focused(cx);
            }

            let focus_handle = cx.focus_handle();
            focus_handle.focus(window);

            root.active_sheet = Some(ActiveSheet {
                focus_handle,
                placement,
                builder: Rc::new(build),
            });
            cx.notify();
        })
    }

    fn has_active_sheet(&mut self, cx: &mut App) -> bool {
        Root::read(self, cx).active_sheet.is_some()
    }

    fn close_sheet(&mut self, cx: &mut App) {
        Root::update(self, cx, |root, window, cx| {
            root.focused_input = None;
            root.active_sheet = None;
            root.focus_back(window, cx);
            cx.notify();
        })
    }

    fn open_dialog<F>(&mut self, cx: &mut App, build: F)
    where
        F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    {
        Root::update(self, cx, move |root, window, cx| {
            // Only save focus handle if there are no active dialogs.
            // This is used to restore focus when all dialogs are closed.
            if root.active_dialogs.len() == 0 {
                root.previous_focus_handle = window.focused(cx);
            }

            let previous_focus = window.focused(cx);
            let focus_handle = cx.focus_handle();
            focus_handle.focus(window);

            root.active_dialogs.push(ActiveDialog {
                focus_handle,
                previous_focus,
                builder: Rc::new(build),
                fade: Rc::new(RefCell::new(None)),
            });
            cx.notify();
        })
    }

    fn has_active_dialog(&mut self, cx: &mut App) -> bool {
        Root::read(self, cx).active_dialogs.len() > 0
    }

    fn close_dialog(&mut self, cx: &mut App) {
        Root::update(self, cx, move |root, window, cx| {
            root.focused_input = None;
            let previous_focus = root.active_dialogs.pop().and_then(|dialog| {
                let focus = dialog.previous_focus.clone();
                root.retain_closing_dialog(dialog, cx);
                focus
            });

            if let Some(top_dialog) = root.active_dialogs.last() {
                // Return to the control that opened the nested dialog.
                previous_focus
                    .as_ref()
                    .unwrap_or(&top_dialog.focus_handle)
                    .focus(window);
            } else {
                // Restore focus if there are no more dialogs.
                root.focus_back(window, cx);
            }
            cx.notify();
        })
    }

    fn close_all_dialogs(&mut self, cx: &mut App) {
        Root::update(self, cx, |root, window, cx| {
            root.focused_input = None;
            for dialog in std::mem::take(&mut root.active_dialogs) {
                root.retain_closing_dialog(dialog, cx);
            }
            root.focus_back(window, cx);
            cx.notify();
        })
    }

    fn push_notification(&mut self, note: impl Into<Notification>, cx: &mut App) {
        let note = note.into();
        Root::update(self, cx, move |root, window, cx| {
            root.notification
                .update(cx, |view, cx| view.push(note, window, cx));
            cx.notify();
        })
    }

    fn remove_notification<T: Sized + 'static>(&mut self, cx: &mut App) {
        Root::update(self, cx, move |root, window, cx| {
            root.notification.update(cx, |view, cx| {
                let id = TypeId::of::<T>();
                view.close(id, window, cx);
            });
            cx.notify();
        })
    }

    fn clear_notifications(&mut self, cx: &mut App) {
        Root::update(self, cx, move |root, window, cx| {
            root.notification
                .update(cx, |view, cx| view.clear(window, cx));
            cx.notify();
        })
    }

    fn notifications(&mut self, cx: &mut App) -> Rc<Vec<Entity<Notification>>> {
        let entity = Root::read(self, cx).notification.clone();
        Rc::new(entity.read(cx).notifications())
    }

    fn has_focused_input(&mut self, cx: &mut App) -> bool {
        Root::read(self, cx).focused_input.is_some()
    }

    fn focused_input(&mut self, cx: &mut App) -> Option<Entity<InputState>> {
        Root::read(self, cx).focused_input.clone()
    }
}

/// Root is a view for the App window for as the top level view (Must be the first view in the window).
///
/// It is used to manage the Sheet, Dialog, and Notification.
pub struct Root {
    /// Used to store the focus handle of the previous view.
    /// When the Dialog, Sheet closes, we will focus back to the previous view.
    previous_focus_handle: Option<FocusHandle>,
    active_sheet: Option<ActiveSheet>,
    pub(crate) active_dialogs: Vec<ActiveDialog>,
    closing_dialogs: Vec<ActiveDialog>,
    pub(super) focused_input: Option<Entity<InputState>>,
    pub notification: Entity<NotificationList>,
    sheet_size: Option<DefiniteLength>,
    view: AnyView,
    _focus_lost_subscription: Subscription,
}

#[derive(Clone)]
struct ActiveSheet {
    focus_handle: FocusHandle,
    placement: Placement,
    builder: Rc<dyn Fn(Sheet, &mut Window, &mut App) -> Sheet + 'static>,
}

#[derive(Clone)]
pub(crate) struct ActiveDialog {
    focus_handle: FocusHandle,
    previous_focus: Option<FocusHandle>,
    builder: Rc<dyn Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static>,
    fade: Rc<RefCell<Option<CssTransition>>>,
}

impl Root {
    /// Create a new Root view.
    pub fn new(view: impl Into<AnyView>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            previous_focus_handle: None,
            active_sheet: None,
            active_dialogs: Vec::new(),
            closing_dialogs: Vec::new(),
            focused_input: None,
            notification: cx.new(|cx| NotificationList::new(window, cx)),
            sheet_size: None,
            view: view.into(),
            _focus_lost_subscription: cx.on_focus_lost(window, |_, window, cx| {
                // A focused control may disappear or become disabled while a modal is
                // open. Restore its container after the application's focus fallback.
                cx.defer_in(window, |root, window, cx| {
                    let focus = root
                        .active_dialogs
                        .last()
                        .map(|dialog| &dialog.focus_handle)
                        .or_else(|| root.active_sheet.as_ref().map(|sheet| &sheet.focus_handle));
                    if let Some(focus) = focus
                        && !focus.contains_focused(window, cx)
                        && (window.focused(cx).is_none() || focus.within_focused(window, cx))
                    {
                        focus.focus(window);
                    }
                });
            }),
        }
    }

    pub fn update<F, R>(window: &mut Window, cx: &mut App, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> R,
    {
        let root = window
            .root::<Root>()
            .flatten()
            .expect("BUG: window first layer should be a gpui_component::Root.");

        root.update(cx, |root, cx| f(root, window, cx))
    }

    pub fn read<'a>(window: &'a Window, cx: &'a App) -> &'a Self {
        &window
            .root::<Root>()
            .expect("The window root view should be of type `ui::Root`.")
            .unwrap()
            .read(cx)
    }

    fn focus_back(&mut self, window: &mut Window, _: &mut App) {
        if let Some(handle) = self.previous_focus_handle.clone() {
            window.focus(&handle);
        }
    }

    // Render Notification layer.
    pub fn render_notification_layer(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement + use<>> {
        let root = window.root::<Root>()??;

        let active_sheet_placement = root.read(cx).active_sheet.clone().map(|d| d.placement);

        let (mt, mr) = match active_sheet_placement {
            Some(Placement::Right) => (None, root.read(cx).sheet_size),
            Some(Placement::Top) => (root.read(cx).sheet_size, None),
            _ => (None, None),
        };

        Some(
            div()
                .absolute()
                .top_0()
                .right_0()
                .when_some(mt, |this, offset| this.mt(offset))
                .when_some(mr, |this, offset| this.mr(offset))
                .child(root.read(cx).notification.clone()),
        )
    }

    /// Render the Sheet layer.
    pub fn render_sheet_layer(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement + use<>> {
        let root = window.root::<Root>()??;

        if let Some(active_sheet) = root.read(cx).active_sheet.clone() {
            let mut sheet = Sheet::new(window, cx);
            sheet = (active_sheet.builder)(sheet, window, cx);
            sheet.focus_handle = active_sheet.focus_handle.clone();
            sheet.placement = active_sheet.placement;

            let size = sheet.size;

            return Some(
                div().relative().child(sheet).child(
                    canvas(
                        move |_, _, cx| root.update(cx, |r, _| r.sheet_size = Some(size)),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                ),
            );
        }

        None
    }

    /// Render the Dialog layer.
    pub fn render_dialog_layer(
        window: &mut Window,
        cx: &mut App,
    ) -> Option<impl IntoElement + use<>> {
        let root = window.root::<Root>()??;

        let closing_count = root.read(cx).closing_dialogs.len();
        let active_dialogs = root
            .read(cx)
            .closing_dialogs
            .iter()
            .chain(&root.read(cx).active_dialogs)
            .cloned()
            .collect::<Vec<_>>();

        if active_dialogs.is_empty() {
            return None;
        }

        let mut show_overlay_ix = None;

        let mut dialogs = active_dialogs
            .iter()
            .enumerate()
            .map(|(i, active_dialog)| {
                let mut dialog = Dialog::new(window, cx);

                dialog = (active_dialog.builder)(dialog, window, cx);

                dialog.id = format!("dialog-{:?}", active_dialog.focus_handle).into();
                dialog.closing = i < closing_count;
                dialog.top_layer = i + 1 == active_dialogs.len();
                if dialog.fade_only {
                    let now = cx.background_executor().now();
                    let mut fade = active_dialog.fade.borrow_mut();
                    let fade = fade.get_or_insert_with(|| {
                        let mut fade = CssTransition::new(
                            0.,
                            Duration::from_millis(300),
                            [0.25, 0.1, 0.25, 1.],
                        );
                        fade.retarget(1., now);
                        fade
                    });
                    let (opacity, animating) = fade.value(now);
                    dialog.fade_opacity = Some(opacity);
                    if animating {
                        window.request_animation_frame();
                    }
                }

                // Give the dialog the focus handle, because `dialog` is a temporary value, is not possible to
                // keep the focus handle in the dialog.
                //
                // So we keep the focus handle in the `active_dialog`, this is owned by the `Root`.
                dialog.focus_handle = active_dialog.focus_handle.clone();

                dialog.layer_ix = i.saturating_sub(closing_count);
                // Find the dialog which one needs to show overlay.
                if dialog.has_overlay() {
                    show_overlay_ix = Some(i);
                }

                dialog
            })
            .collect::<Vec<_>>();

        if let Some(ix) = show_overlay_ix {
            if let Some(dialog) = dialogs.get_mut(ix) {
                dialog.overlay_visible = true;
            }
        }

        Some(div().children(dialogs))
    }

    fn retain_closing_dialog(&mut self, dialog: ActiveDialog, cx: &mut Context<Self>) {
        let mut fade = dialog.fade.borrow_mut();
        let Some(fade) = fade.as_mut() else { return };
        fade.retarget(0., cx.background_executor().now());
        let focus = dialog.focus_handle.clone();
        self.closing_dialogs.push(dialog.clone());
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            this.update(cx, |root, cx| {
                root.closing_dialogs
                    .retain(|dialog| dialog.focus_handle != focus);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Return the root view of the Root.
    pub fn view(&self) -> &AnyView {
        &self.view
    }

    fn on_action_tab(&mut self, _: &Tab, window: &mut Window, _: &mut Context<Self>) {
        window.focus_next();
    }

    fn on_action_tab_prev(&mut self, _: &TabPrev, window: &mut Window, _: &mut Context<Self>) {
        window.focus_prev();
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(cx.theme().font_size);

        window_border().child(
            div()
                .id("root")
                .key_context(CONTEXT)
                .on_action(cx.listener(Self::on_action_tab))
                .on_action(cx.listener(Self::on_action_tab_prev))
                .relative()
                .size_full()
                .font_family(cx.theme().font_family.clone())
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .child(self.view.clone()),
        )
    }
}
