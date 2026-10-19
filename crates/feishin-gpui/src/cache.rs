use crate::app::Feishin;
use gpui::{prelude::*, *};
use gpui_component::{Disableable, StyledExt};

impl Feishin {
    fn cache_label(&self, full: bool) -> String {
        feishin_core::i18n::translate(
            &self.state.settings.language,
            if full {
                "setting.clearCache"
            } else {
                "setting.clearQueryCache"
            },
        )
        .unwrap_or_else(|| {
            if full {
                "Clear cache"
            } else {
                "Clear query cache"
            }
            .into()
        })
    }
    pub fn cache_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .h_flex()
            .gap_3()
            .children([false, true].map(|full| {
                let id = if full {
                    "clear-native-cache"
                } else {
                    "clear-query-cache"
                };
                crate::styles::button(id, cx)
                    .debug_selector(move || id.into())
                    .label(self.cache_label(full))
                    .disabled(self.cache_busy)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.confirm_cache_clear(full, window, cx)
                    }))
            }))
            .into_any_element()
    }
    pub fn confirm_cache_clear(&mut self, full: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.cache_busy {
            return;
        }
        let confirm = feishin_core::i18n::translate(&self.state.settings.language, "common.clear")
            .unwrap_or_else(|| "Clear".into());
        let cancel = feishin_core::i18n::translate(&self.state.settings.language, "common.cancel")
            .unwrap_or_else(|| "Cancel".into());
        let question =
            feishin_core::i18n::translate(&self.state.settings.language, "common.areYouSure")
                .unwrap_or_else(|| "Are you sure?".into());
        let confirmation = window.prompt(
            PromptLevel::Warning,
            &self.cache_label(full),
            Some(&question),
            &[confirm.as_str(), cancel.as_str()],
            cx,
        );
        cx.spawn(async move |view, cx| {
            if confirmation.await.ok() == Some(0) {
                view.update(cx, |view, cx| view.clear_cache(full, cx)).ok();
            }
        })
        .detach();
    }
    pub(crate) fn invalidate_library_data(&mut self) {
        self.navigation_restore = None;
        self.album_related_generation = self.album_related_generation.wrapping_add(1);
        self.album_related.clear();
        self.query_cache_generation = self.query_cache_generation.wrapping_add(1);
        self.request_generation += 1;
        self.items.clear();
        self.random_sample = None;
        self.total = None;
        self.has_more = false;
        self.loading = false;
        self.selected.clear();
        self.grid_selection_order.clear();
        self.selected_album_tracks.clear();
        self.album_tracks_generation += 1;
        self.album_tracks.clear();
        self.album_metadata_cache.clear();
        self.artist_section_generation += 1;
        self.artist_sections.clear();
        self.artist_info = None;
        self.artist_info_target = None;
        self.artist_info_error.clear();
        self.home_generation += 1;
        self.home_sections.clear();
        self.reset_playlists();
        self.info_generation += 1;
        self.info_items.clear();
        self.info_loading = false;
    }
    fn clear_query_data(&mut self) {
        self.invalidate_library_data();
        for api in self.sessions.values() {
            api.clear_query_cache();
        }
        self.presence_artwork.clear();
        self.tag_suggestion_generation += 1;
        self.tag_suggestions = None;
        self.tag_source_names.clear();
        self.queue_drawer_generation += 1;
        self.filter_generation += 1;
        self.lyric_generation += 1;
        self.lyric_busy = false;
        self.lyric_record = None;
        self.lyric_remote_suppressed.clear();
        self.lyric_selections.clear();
        self.lyrics.clear();
        self.lyric_translation_generation += 1;
        self.lyric_translation_busy = false;
        self.lyric_translated.clear();
        self.annotation_generation += 1;
        self.lyric_annotations.clear();
    }
    pub fn clear_cache(&mut self, full: bool, cx: &mut Context<Self>) {
        if self.cache_busy {
            return;
        }
        self.clear_query_data();
        if !full {
            self.reload(false, cx);
            self.load_lyrics(true, cx);
            self.message = feishin_core::i18n::translate(
                &self.state.settings.language,
                "setting.clearCacheSuccess",
            )
            .unwrap_or_else(|| "Cache cleared".into());
            cx.notify();
            return;
        }
        self.cache_busy = true;
        self.state.cache_revision = self.state.cache_revision.wrapping_add(1).max(1);
        let paths: Vec<_> = self
            .covers
            .values()
            .chain(self.censored_covers.values())
            .chain(
                self.player_background
                    .as_ref()
                    .map(|background| &background.image),
            )
            .chain(
                self.detail_background
                    .as_ref()
                    .map(|background| &background.image),
            )
            .chain(
                self.home_backgrounds
                    .values()
                    .filter_map(Option::as_ref)
                    .map(|background| &background.image),
            )
            .cloned()
            .collect();
        for path in paths {
            ImageSource::from(path).remove_asset(cx);
        }
        self.covers.clear();
        self.cover_versions.clear();
        self.censored_covers.clear();
        *self.image_placeholders.borrow_mut() = Default::default();
        self.player_background = None;
        self.player_background_key.clear();
        self.detail_background = None;
        self.detail_background_key.clear();
        self.home_backgrounds.clear();
        self.reset_waveform();
        self.save();
        let root = self.data_dir.clone();
        let protected: Vec<_> = self
            .state
            .radio
            .iter()
            .map(|item| {
                root.join("covers")
                    .join(format!("{}.img", Self::cover_key(item)))
            })
            .collect();
        let task = cx
            .background_executor()
            .spawn(async move { feishin_core::cache::clear_disk(&root, &protected) });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                view.cache_busy = false;
                view.reload(false, cx);
                view.fetch_covers(cx);
                view.load_lyrics(true, cx);
                view.message = match result {
                    Ok(_) => feishin_core::i18n::translate(
                        &view.state.settings.language,
                        "setting.clearCacheSuccess",
                    )
                    .unwrap_or_else(|| "Cache cleared".into()),
                    Err(error) => {
                        log::warn!("Native cache clear failed: {error:#}");
                        format!("Could not clear cache: {error:#}")
                    }
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    use feishin_core::{
        model::{Item, ItemKind},
        queue::Enqueue,
        storage::State,
    };
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn stale_artwork_download_cannot_repopulate_a_cleared_cache(cx: &mut TestAppContext) {
        use feishin_core::{
            api::Api,
            images::Placement,
            model::{Server, ServerType, Session},
        };
        use std::{
            io::{Read, Write},
            time::Duration,
        };
        cx.update(gpui_component::init);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 8192];
            let n = socket.read(&mut request).unwrap();
            assert!(String::from_utf8_lossy(&request[..n]).contains("/rest/getCoverArt.view?"));
            let body = b"obsolete artwork";
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            socket.write_all(body).unwrap();
        });
        let root =
            std::env::temp_dir().join(format!("feishin-cache-race-{}", uuid::Uuid::new_v4()));
        let server = uuid::Uuid::new_v4();
        let item = Item {
            id: "track".into(),
            server_id: server,
            cover_id: "art".into(),
            ..Default::default()
        };
        let captured = Rc::new(RefCell::new(None));
        let original = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(root.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.route = crate::app::Route::Settings;
                view.state.settings.settings_tab = feishin_core::settings_search::Tab::Advanced;
                view.sessions.insert(
                    server,
                    Api::new(
                        Server {
                            id: server,
                            name: "Fixture".into(),
                            url: format!("http://{address}"),
                            kind: ServerType::Subsonic,
                            username: "fixture".into(),
                            legacy_auth: false,
                            options: Default::default(),
                        },
                        Session::default(),
                    )
                    .unwrap(),
                );
                view.fetch_item_cover_at(&item, Placement::Table, cx);
                original.replace(Some(root.join("covers").join(format!(
                    "{}.img",
                    view.cover_cache_key(&item, Placement::Table)
                ))));
                // Queue the clear before GPUI dispatches the blocking HTTP job,
                // then let the real response finish against its old revision.
                view.clear_cache(true, cx);
            });
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        let old_path = original.borrow_mut().take().unwrap();
        cx.run_until_parked();
        assert!(!app.read_with(cx, |view, _| view.cache_busy));
        assert!(!old_path.exists());
        worker.join().unwrap();
        cx.run_until_parked();
        // The callback also removes any file written after disk pruning ended.
        assert!(!old_path.exists());
        assert!(!app.read_with(cx, |view, _| view.covers.contains_key(
            &feishin_core::images::cache_key(&item, view.state.settings.image_resolution.table)
        )));
        assert_ne!(
            old_path,
            app.read_with(cx, |view, _| root.join("covers").join(format!(
                "{}.img",
                view.cover_cache_key(&item, Placement::Table)
            )))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[gpui::test]
    fn cache_buttons_confirm_cancel_clear_and_preserve_queue_overrides_local_radio_and_preferences(
        cx: &mut TestAppContext,
    ) {
        struct Probe(Entity<Feishin>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.0.update(cx, |view, cx| view.cache_controls(cx))
            }
        }
        cx.update(gpui_component::init);
        let root = std::env::temp_dir().join(format!("feishin-cache-ui-{}", uuid::Uuid::new_v4()));
        let radio = Item {
            id: "local-radio".into(),
            kind: ItemKind::Radio,
            ..Default::default()
        };
        let radio_path = root
            .join("covers")
            .join(format!("{}.img", Feishin::cover_key(&radio)));
        for file in [
            "covers/old.img",
            "waveforms/old.json",
            "lyrics/cache/old.json",
            "lyrics/overrides/keep.json",
            "themes/keep.json",
            "custom.css",
        ] {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file).unwrap();
        }
        std::fs::write(&radio_path, "local radio source").unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.volume = 73.;
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.radio.push(radio.clone());
            state.queue.add(
                vec![
                    Item {
                        id: "duplicate".into(),
                        ..Default::default()
                    },
                    Item {
                        id: "duplicate".into(),
                        ..Default::default()
                    },
                ],
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(root.clone(), Ok(state), window, cx));
            app.update(cx, |view, _| {
                view.route = crate::app::Route::Settings;
                view.state.settings.settings_tab = feishin_core::settings_search::Tab::Advanced;
                view.items.push(Item {
                    id: "cached-library".into(),
                    ..Default::default()
                });
                view.covers
                    .insert("old".into(), root.join("covers/old.img"));
                view.album_metadata_cache.insert("album".into(), None);
            });
            captured.replace(Some(app.clone()));
            let probe = cx.new(|_| Probe(app));
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let original_queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        let click = |selector, cx: &mut VisualTestContext| {
            let point = cx.debug_bounds(selector).unwrap().center();
            cx.simulate_click(point, Modifiers::none());
            cx.run_until_parked();
            assert!(cx.has_pending_prompt());
        };
        click("clear-native-cache", cx);
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert!(root.join("covers/old.img").exists());
        assert_eq!(app.read_with(cx, |view, _| view.state.cache_revision), 0);
        click("clear-query-cache", cx);
        cx.simulate_prompt_answer("Clear");
        cx.run_until_parked();
        assert!(app.read_with(cx, |view, _| view.items.is_empty()
            && view.album_metadata_cache.is_empty()));
        assert!(root.join("covers/old.img").exists());
        assert_eq!(app.read_with(cx, |view, _| view.state.cache_revision), 0);
        click("clear-native-cache", cx);
        cx.simulate_prompt_answer("Clear");
        cx.run_until_parked();
        assert!(!app.read_with(cx, |view, _| view.cache_busy));
        assert!(!root.join("covers/old.img").exists());
        assert!(!root.join("waveforms/old.json").exists());
        assert!(!root.join("lyrics/cache/old.json").exists());
        assert_eq!(
            std::fs::read_to_string(&radio_path).unwrap(),
            "local radio source"
        );
        for file in [
            "lyrics/overrides/keep.json",
            "themes/keep.json",
            "custom.css",
        ] {
            assert_eq!(std::fs::read_to_string(root.join(file)).unwrap(), file);
        }
        let state = feishin_core::storage::load(&root.join("state.json")).unwrap();
        assert_eq!(state.settings.volume, 73.);
        assert_eq!(state.cache_revision, 1);
        assert_eq!(serde_json::to_value(&state.queue).unwrap(), original_queue);
        assert_eq!(
            app.read_with(cx, |view, _| serde_json::to_value(&view.state.queue)
                .unwrap()),
            original_queue
        );
        assert!(
            app.read_with(cx, |view, _| view.cover_cache_key(
                &Item::default(),
                feishin_core::images::Placement::Table
            ))
            .ends_with("-cache-1")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
