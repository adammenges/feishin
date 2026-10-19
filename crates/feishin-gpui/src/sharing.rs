use crate::app::{Feishin, Route};
use chrono::{Local, TimeZone};
use feishin_core::{
    model::{Item, ItemKind, ServerType},
    storage,
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, StyledExt,
    button::ButtonVariants,
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
};

impl Feishin {
    pub fn open_share(&mut self, items: Vec<Item>, window: &mut Window, cx: &mut Context<Self>) {
        if items.is_empty() {
            return;
        }
        self.share_items = items;
        self.share_url = None;
        self.share_description
            .update(cx, |input, cx| input.set_value("", window, cx));
        let expiration = self
            .state
            .settings
            .share_expiration
            .clone()
            .unwrap_or_else(|| {
                let (amount, unit) = match self.state.settings.share_days {
                    Some(30) => (1, feishin_core::sharing::Unit::Month),
                    Some(365) => (1, feishin_core::sharing::Unit::Year),
                    Some(days) => (days.max(1), feishin_core::sharing::Unit::Day),
                    None => (1, feishin_core::sharing::Unit::Year),
                };
                feishin_core::sharing::Expiration {
                    amount,
                    unit,
                    use_server_default: self.state.settings.share_days.is_none(),
                }
            });
        let date = if self.state.settings.share_expiration.is_none()
            && self.state.settings.share_days == Some(0)
        {
            "never".into()
        } else {
            match expiration.deadline(Local::now()) {
                Ok(date) => date
                    .map(|date| date.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default(),
                Err(error) => {
                    self.message = error.to_string();
                    return;
                }
            }
        };
        if self.state.settings.share_days != Some(0) {
            self.state.settings.share_expiration = Some(expiration.clone());
        }
        self.share_amount.update(cx, |input, cx| {
            input.set_value(expiration.amount.to_string(), window, cx)
        });
        self.share_expiry
            .update(cx, |input, cx| input.set_value(date, window, cx));
        self.navigate(Route::Share, cx);
    }
    pub fn apply_share_expiration(
        &mut self,
        unit: Option<feishin_core::sharing::Unit>,
        server_default: Option<bool>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(amount) = self.share_amount.read(cx).value().parse::<u32>() else {
            self.message = "Enter a positive share expiration amount".into();
            cx.notify();
            return;
        };
        let mut expiration = self
            .state
            .settings
            .share_expiration
            .clone()
            .unwrap_or_default();
        expiration.amount = amount;
        if let Some(unit) = unit {
            expiration.unit = unit;
        }
        if let Some(value) = server_default {
            expiration.use_server_default = value;
        }
        match expiration.deadline(Local::now()) {
            Ok(date) => {
                self.share_expiry.update(cx, |input, cx| {
                    input.set_value(
                        date.map(|date| date.format("%Y-%m-%d %H:%M:%S").to_string())
                            .unwrap_or_default(),
                        window,
                        cx,
                    )
                });
                self.state.settings.share_expiration = Some(expiration);
                self.save();
            }
            Err(error) => self.message = error.to_string(),
        }
        cx.notify();
    }

    fn create_share(&mut self, cx: &mut Context<Self>) {
        if self.share_busy {
            return;
        }
        let Some(api) = self
            .share_items
            .first()
            .and_then(|item| self.sessions.get(&item.server_id))
            .cloned()
        else {
            return;
        };
        let expiry = self.share_expiry.read(cx).value().trim().to_owned();
        let expires = if expiry.is_empty() {
            None
        } else if expiry.eq_ignore_ascii_case("never") {
            Some(0)
        } else {
            let date = chrono::NaiveDateTime::parse_from_str(&expiry, "%Y-%m-%d %H:%M:%S")
                .ok()
                .and_then(|date| Local.from_local_datetime(&date).single());
            let Some(date) = date.filter(|date| date.timestamp_millis() > 0) else {
                self.message = "Enter a local date as YYYY-MM-DD HH:MM:SS".into();
                cx.notify();
                return;
            };
            Some(date.timestamp_millis() as u64)
        };
        let description = self.share_description.read(cx).value().to_string();
        let items = self.share_items.clone();
        let downloadable = self.state.settings.share_downloadable;
        self.share_busy = true;
        let task = cx
            .background_executor()
            .spawn(async move { api.share(&items, &description, downloadable, expires) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.share_busy = false;
                match result {
                    Ok(url) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                        this.share_url = Some(url);
                        this.message = "Share link copied".into();
                    }
                    Err(error) => this.message = error.to_string(),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub fn share_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let expiration = self
            .state
            .settings
            .share_expiration
            .clone()
            .unwrap_or_default();
        gpui::div()
            .v_flex()
            .gap_4()
            .max_w(px(750.0))
            .child(format!("Share {} selected item(s)", self.share_items.len()))
            .child(crate::styles::input(&self.share_description, cx))
            .child(crate::styles::input(&self.share_expiry, cx))
            .child(
                Checkbox::new("share-server-default")
                    .label(self.tr("Server default"))
                    .checked(expiration.use_server_default)
                    .on_click(cx.listener(|this, value: &bool, window, cx| {
                        this.apply_share_expiration(None, Some(*value), window, cx)
                    })),
            )
            .child(
                gpui::div()
                    .h_flex()
                    .gap_2()
                    .child(
                        gpui::div()
                            .w(px(140.))
                            .child(crate::styles::number_input(&self.share_amount, cx)),
                    )
                    .child(
                        crate::styles::button("share-calendar-unit", cx)
                            .label(self.tr(expiration.unit.label()))
                            .dropdown_menu({
                                let view = cx.weak_entity();
                                move |mut menu, _, cx| {
                                    for unit in feishin_core::sharing::Unit::ALL {
                                        let view = view.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(crate::appearance::text(
                                                unit.label(),
                                                cx,
                                            ))
                                            .on_click(move |_, window, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.apply_share_expiration(
                                                        Some(unit),
                                                        None,
                                                        window,
                                                        cx,
                                                    )
                                                })
                                                .ok();
                                            }),
                                        );
                                    }
                                    menu
                                }
                            }),
                    )
                    .child(
                        crate::styles::button("apply-share-calendar", cx)
                            .label(self.tr("common.confirm"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.apply_share_expiration(None, None, window, cx)
                            })),
                    ),
            )
            .child(
                gpui::div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .flex_wrap()
                    .children(
                        [
                            (None, "Server default"),
                            (Some(0), "Never"),
                            (Some(1), "1 day"),
                            (Some(7), "1 week"),
                            (Some(30), "1 month"),
                            (Some(365), "1 year"),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(index, (days, label))| {
                            crate::styles::button(("share-expiry", index), cx)
                                .label(self.tr(label))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.state.settings.share_days = days;
                                    this.state.settings.share_expiration = if days == Some(0) {
                                        None
                                    } else {
                                        let (amount, unit) = match days {
                                            Some(30) => (1, feishin_core::sharing::Unit::Month),
                                            Some(365) => (1, feishin_core::sharing::Unit::Year),
                                            Some(7) => (1, feishin_core::sharing::Unit::Week),
                                            Some(days) => (days, feishin_core::sharing::Unit::Day),
                                            None => (1, feishin_core::sharing::Unit::Year),
                                        };
                                        this.share_amount.update(cx, |input, cx| {
                                            input.set_value(amount.to_string(), window, cx)
                                        });
                                        Some(feishin_core::sharing::Expiration {
                                            amount,
                                            unit,
                                            use_server_default: days.is_none(),
                                        })
                                    };
                                    let now = Local::now();
                                    let value = days.map_or(String::new(), |days| {
                                        if days == 0 {
                                            "never".into()
                                        } else {
                                            let date = match days {
                                                30 => now
                                                    .checked_add_months(chrono::Months::new(1))
                                                    .unwrap_or(now),
                                                365 => now
                                                    .checked_add_months(chrono::Months::new(12))
                                                    .unwrap_or(now),
                                                _ => now + chrono::Duration::days(days as i64),
                                            };
                                            date.format("%Y-%m-%d %H:%M:%S").to_string()
                                        }
                                    });
                                    this.share_expiry
                                        .update(cx, |input, cx| input.set_value(value, window, cx));
                                    this.save();
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(
                crate::styles::button("share-downloadable", cx)
                    .label(format!(
                        "Allow downloading: {}",
                        self.state.settings.share_downloadable
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.share_downloadable =
                            !this.state.settings.share_downloadable;
                        this.save();
                        cx.notify();
                    })),
            )
            .child(
                crate::styles::button("share-create", cx)
                    .primary()
                    .label(self.tr("Create share link"))
                    .disabled(self.share_busy)
                    .on_click(cx.listener(|this, _, _, cx| this.create_share(cx))),
            )
            .when_some(self.share_url.clone(), |div, url| {
                let copy = url.clone();
                let open = url.clone();
                div.child(gpui::div().child(url)).child(
                    gpui::div()
                        .h_flex()
                        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                        .gap_2()
                        .child(
                            crate::styles::button("copy-share", cx)
                                .label(crate::appearance::text("Copy link", cx))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                                })),
                        )
                        .child(
                            crate::styles::button("open-share", cx)
                                .label(crate::appearance::text("Open link", cx))
                                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&open))),
                        ),
                )
            })
            .into_any_element()
    }
    pub fn can_upload_library_image(&self, item: &Item) -> bool {
        matches!(
            item.kind,
            ItemKind::Artist | ItemKind::AlbumArtist | ItemKind::Playlist
        ) && self.sessions.get(&item.server_id).is_some_and(|api| {
            matches!(
                api.server.kind,
                ServerType::Navidrome | ServerType::Jellyfin
            )
        })
    }

    pub fn choose_library_image(
        &mut self,
        item: Item,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select artwork".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picker.await
                && let Some(path) = paths.first()
            {
                let path = path.clone();
                this.update_in(cx, |this, _, cx| {
                    this.change_library_image(item, Some(path), cx)
                })
                .ok();
            }
        })
        .detach();
    }
    pub fn change_library_image(
        &mut self,
        item: Item,
        path: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.sessions.get(&item.server_id).cloned() else {
            return;
        };
        let key = Self::cover_key(&item);
        let cache = self.data_dir.join("covers").join(format!("{key}.img"));
        let local = item.kind == feishin_core::model::ItemKind::Radio
            && api.server.kind == ServerType::Jellyfin;
        let task = cx.background_executor().spawn(async move {
            if let Some(path) = path {
                use std::io::Read;
                let mut bytes = vec![];
                std::fs::File::open(path)?
                    .take(20_000_001)
                    .read_to_end(&mut bytes)?;
                if api.server.kind == ServerType::Jellyfin
                    && item.kind == feishin_core::model::ItemKind::Radio
                {
                    anyhow::ensure!(bytes.len() <= 20_000_000, "Image is too large");
                    storage::atomic_write(&cache, &bytes)?;
                } else {
                    api.upload_image(&item, bytes)?;
                    let _ = std::fs::remove_file(cache);
                }
            } else {
                if !(api.server.kind == ServerType::Jellyfin
                    && item.kind == feishin_core::model::ItemKind::Radio)
                {
                    api.remove_image(&item)?;
                }
                let _ = std::fs::remove_file(cache);
            }
            Ok::<_, anyhow::Error>(())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.invalidate_artwork(&key, local);
                        this.fetch_covers(cx);
                        this.message = "Artwork updated".into();
                    }
                    Err(error) => {
                        log::warn!("Artwork update failed: {error}");
                        this.message = error.to_string();
                    }
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
