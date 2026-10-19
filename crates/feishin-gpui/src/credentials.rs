use crate::app::Feishin;
use feishin_core::credentials::Backend;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement};
use gpui_component::menu::{DropdownMenu, PopupMenuItem};

impl Feishin {
    pub fn password_store_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.weak_entity();
        crate::styles::button("password-store", cx)
            .debug_selector(|| "password-store".into())
            .label(format!(
                "{}: {}",
                feishin_core::i18n::translate(
                    &self.state.settings.language,
                    "setting.passwordStore"
                )
                .unwrap_or_else(|| "Passwords/secret store".into()),
                self.state.settings.password_store.label()
            ))
            .dropdown_menu(move |mut menu, _, _| {
                for (backend, label) in Backend::ALL {
                    let view = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                        view.update(cx, |view, cx| {
                            view.state.settings.password_store = backend;
                            view.save();
                            view.message = view.tr("Restart required");
                            cx.notify();
                        })
                        .ok();
                    }));
                }
                menu
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feishin_core::storage::State;
    use gpui::{AppContext, Entity, Modifiers, Render, TestAppContext, Window};
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn password_store_mouse_choices_persist_without_accessing_the_running_keychain(
        cx: &mut TestAppContext,
    ) {
        struct Probe(Entity<Feishin>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.0
                    .update(cx, |view, cx| view.password_store_control(cx))
            }
        }
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!(
            "feishin-password-store-ui-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            let probe = cx.new(|_| Probe(app));
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        for (index, (backend, _)) in Backend::ALL.iter().enumerate() {
            let trigger = cx.debug_bounds("password-store").unwrap().center();
            cx.simulate_click(trigger, Modifiers::none());
            cx.run_until_parked();
            let selector = match index {
                0 => "popup-menu-item-0",
                1 => "popup-menu-item-1",
                2 => "popup-menu-item-2",
                _ => "popup-menu-item-3",
            };
            let item = cx.debug_bounds(selector).unwrap().center();
            cx.simulate_click(item, Modifiers::none());
            cx.run_until_parked();
            assert_eq!(
                app.read_with(cx, |view, _| view.state.settings.password_store),
                *backend
            );
            assert_eq!(
                feishin_core::storage::load(&dir.join("state.json"))
                    .unwrap()
                    .settings
                    .password_store,
                *backend
            );
            assert!(!app.read_with(cx, |view, _| view.message.is_empty()));
        }
        std::fs::remove_dir_all(dir).ok();
    }
}
