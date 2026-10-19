use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{model::ItemKind, presence, storage};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, StyledExt,
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
};
use std::time::{Duration, Instant};

const DISPLAY_TYPES: [(&str, &str); 3] = [
    ("feishin", "Feishin"),
    ("song", "setting.discordDisplayType_songname"),
    ("artist", "setting.discordDisplayType_artistname"),
];
const LINK_TYPES: [(&str, &str); 4] = [
    ("none", "setting.discordLinkType_none"),
    ("last_fm", "last.fm"),
    ("musicbrainz", "musicbrainz"),
    ("musicbrainz_last_fm", "setting.discordLinkType_mbz_lastfm"),
];

pub(crate) fn text(key: &str, cx: &App) -> String {
    let language = cx
        .try_global::<crate::appearance::Language>()
        .map_or("en", |locale| locale.0.as_str());
    feishin_core::i18n::translate_with(language, key, &serde_json::json!({
        "discord": "Discord", "musicbrainz": "musicbrainz",
        "lastfm": if key.starts_with("setting.lastfmApiKey") { "Last.fm" } else { "last.fm" },
        "defaultId": "1165957668758900787", "icon": "icon", "playing": "playing", "paused": "paused"
    })).unwrap_or_else(|| crate::appearance::text(key, cx))
}

impl Feishin {
    pub fn sync_presence_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.presence_settings_inputs[0].update(cx, |input, cx| {
            input.set_value(self.state.settings.discord_client_id.clone(), window, cx)
        });
        self.presence_settings_inputs[1].update(cx, |input, cx| input.set_value("", window, cx));
    }

    pub fn update_presence(&mut self, cx: &mut Context<Self>) {
        let item = (self.loaded || self.playing || self.buffering)
            .then(|| self.current_metadata())
            .flatten();
        let config = &self.state.settings.discord_config;
        let mut cover = None;
        if self.state.settings.discord_enabled
            && !self.state.settings.private_mode
            && let Some(item) = item.as_ref().filter(|item| item.kind == ItemKind::Song)
        {
            let api = self.sessions.get(&item.server_id).cloned();
            let signature = format!(
                "{}:{}:{}:{}:{}:{:?}",
                item.server_id,
                item.album_id,
                item.cover_id,
                config.show_server_image,
                api.as_ref()
                    .map(|api| format!(
                        "{}:{:?}",
                        api.server.options.remote_url,
                        feishin_core::api::cover_hash(&api.session().token)
                    ))
                    .unwrap_or_default(),
                feishin_core::api::cover_hash(&config.last_fm_api_key)
            );
            if let Some((_, url)) = self
                .presence_artwork
                .get(&signature)
                .filter(|(expires, _)| *expires > Instant::now())
            {
                cover = url.clone();
            } else if config.show_server_image || !config.last_fm_api_key.is_empty() {
                if self.presence_artwork.len() >= 256 {
                    self.presence_artwork.clear();
                }
                self.presence_artwork.insert(
                    signature.clone(),
                    (Instant::now() + Duration::from_secs(30), None),
                );
                let item = item.clone();
                let config = config.clone();
                let task = cx.background_executor().spawn(async move {
                    let server = if config.show_server_image {
                        api.and_then(|api| api.presence_image(&item).ok().flatten())
                    } else {
                        None
                    };
                    server.or_else(|| {
                        presence::lastfm_image(
                            &config.last_fm_api_key,
                            item.primary_album_artist(),
                            &item.album,
                        )
                        .ok()
                        .flatten()
                    })
                });
                cx.spawn(async move |this, cx| {
                    let result = task.await;
                    this.update(cx, |this, cx| {
                        let expiry = Instant::now()
                            + Duration::from_secs(if result.is_some() { 3600 } else { 30 });
                        if this.presence_artwork.len() >= 256
                            && !this.presence_artwork.contains_key(&signature)
                        {
                            this.presence_artwork.clear();
                        }
                        this.presence_artwork.insert(signature, (expiry, result));
                        this.update_presence(cx);
                    })
                    .ok();
                })
                .detach();
            }
        }
        self.presence.publish_with_cover(
            item.as_ref(),
            self.playing,
            self.current_position(),
            &self.state.settings,
            cover.as_deref(),
        );
    }

    pub fn presence_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.state.settings.discord_config;
        let description = |key: &str| {
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(text(key, cx))
        };
        div()
            .id("presence-settings")
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .child(
                crate::styles::button("presence-back", cx)
                    .label(self.tr("Back"))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Settings, cx))),
            )
            .child(div().text_lg().child(text("page.setting.discord", cx)))
            .children(
                [
                    (
                        "setting.discordRichPresence",
                        0,
                        self.state.settings.discord_enabled,
                    ),
                    ("setting.discordListening", 1, config.show_as_listening),
                    ("setting.discordPausedStatus", 2, config.show_paused),
                    ("setting.discordServeImage", 3, config.show_server_image),
                    ("setting.discordStateIcon", 4, config.show_state_icon),
                ]
                .into_iter()
                .map(|(label, index, checked)| {
                    Checkbox::new(("presence-option", index as usize))
                        .debug_selector(move || format!("presence-option-{index}"))
                        .label(text(label, cx))
                        .child(description(&format!("{label}_description")))
                        .when(crate::appearance::rtl(cx), |checkbox| {
                            checkbox.flex_row_reverse()
                        })
                        .checked(checked)
                        .on_click(cx.listener(move |this, value: &bool, _, cx| {
                            match index {
                                0 => this.state.settings.discord_enabled = *value,
                                1 => this.state.settings.discord_config.show_as_listening = *value,
                                2 => this.state.settings.discord_config.show_paused = *value,
                                3 => this.state.settings.discord_config.show_server_image = *value,
                                _ => this.state.settings.discord_config.show_state_icon = *value,
                            }
                            this.save();
                            this.update_presence(cx);
                            cx.notify();
                        }))
                }),
            )
            .child(
                crate::styles::button("presence-display-type", cx)
                    .debug_selector(|| "presence-display-type".into())
                    .label(format!(
                        "{}: {}",
                        text("setting.discordDisplayType", cx),
                        text(
                            DISPLAY_TYPES
                                .iter()
                                .find(|(value, _)| *value == config.display_type)
                                .map_or("Feishin", |(_, label)| label),
                            cx
                        )
                    ))
                    .dropdown_menu({
                        let view = cx.weak_entity();
                        let selected = config.display_type.clone();
                        move |mut menu, _, cx| {
                            for (value, label) in DISPLAY_TYPES {
                                let view = view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(text(label, cx))
                                        .checked(value == selected)
                                        .on_click(move |_, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.settings.discord_config.display_type =
                                                    value.into();
                                                this.save();
                                                this.update_presence(cx);
                                                cx.notify();
                                            })
                                            .ok();
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            .child(description("setting.discordDisplayType_description"))
            .child(
                crate::styles::button("presence-link-type", cx)
                    .debug_selector(|| "presence-link-type".into())
                    .label(format!(
                        "{}: {}",
                        text("setting.discordLinkType", cx),
                        text(
                            LINK_TYPES
                                .iter()
                                .find(|(value, _)| *value == config.link_type)
                                .map_or("setting.discordLinkType_none", |(_, label)| label),
                            cx
                        )
                    ))
                    .dropdown_menu({
                        let view = cx.weak_entity();
                        let selected = config.link_type.clone();
                        move |mut menu, _, cx| {
                            for (value, label) in LINK_TYPES {
                                let view = view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(text(label, cx))
                                        .checked(value == selected)
                                        .on_click(move |_, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.settings.discord_config.link_type =
                                                    value.into();
                                                this.save();
                                                this.update_presence(cx);
                                                cx.notify();
                                            })
                                            .ok();
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            .child(description("setting.discordLinkType_description"))
            .child(text("setting.discordApplicationId", cx))
            .child(crate::styles::input(&self.presence_settings_inputs[0], cx))
            .child(description("setting.discordApplicationId_description"))
            .child(text("setting.lastfmApiKey", cx))
            .child(crate::styles::input(&self.presence_settings_inputs[1], cx).mask_toggle())
            .child(description("setting.lastfmApiKey_description"))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.tr("Leave the API key empty to keep the saved key.")),
            )
            .child(
                crate::styles::button("presence-apply", cx)
                    .debug_selector(|| "presence-apply".into())
                    .label(self.tr("common.save"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let id = this.presence_settings_inputs[0]
                            .read(cx)
                            .value()
                            .trim()
                            .to_owned();
                        if id.is_empty() || id.len() > 32 || !id.bytes().all(|n| n.is_ascii_digit())
                        {
                            this.message = "Enter a numeric Discord client ID".into();
                            cx.notify();
                            return;
                        }
                        let key = this.presence_settings_inputs[1]
                            .read(cx)
                            .value()
                            .to_string();
                        if !key.is_empty() {
                            if let Err(error) = storage::store_lastfm_key(&key) {
                                this.message = format!("Could not save Last.fm key: {error}");
                                cx.notify();
                                return;
                            }
                            this.state.settings.discord_config.last_fm_api_key = key;
                            this.state.settings.discord_config.last_fm_key_enabled = Some(true);
                        }
                        this.state.settings.discord_client_id = id;
                        this.sync_presence_inputs(window, cx);
                        this.presence_artwork.clear();
                        this.save();
                        this.update_presence(cx);
                        cx.notify();
                    })),
            )
            .child(
                crate::styles::button("presence-remove-key", cx)
                    .label(self.tr("Remove Last.fm API key"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        match storage::store_lastfm_key("") {
                            Ok(()) => {
                                this.state.settings.discord_config.last_fm_api_key.clear();
                                this.state.settings.discord_config.last_fm_key_enabled =
                                    Some(false);
                                this.presence_artwork.clear();
                                this.update_presence(cx);
                                this.save();
                            }
                            Err(error) => {
                                this.message = format!("Could not remove Last.fm key: {error}")
                            }
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
