use crate::app::*;
use feishin_core::{
    images::{self, Placeholder, Placement},
    model::*,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::ButtonVariants,
    menu::{DropdownMenu, PopupMenuItem},
};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
};

pub(crate) fn first_image_path(paths: &[PathBuf]) -> Option<&PathBuf> {
    paths.iter().find(|path| {
        path.is_file()
            && mime_guess::from_path(path)
                .iter()
                .any(|mime| mime.type_() == "image")
    })
}

#[derive(Default)]
pub struct PlaceholderCache {
    entries: HashMap<String, Option<Arc<RenderImage>>>,
    order: VecDeque<String>,
}

struct ArtworkDialog {
    parent: Entity<Feishin>,
    item: Item,
    _subscription: Subscription,
}

impl Render for ArtworkDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let parent = self.parent.read(cx);
        let zoom = parent.state.settings.zoom;
        let radius = cx.theme().font_size * 0.3125;
        let explicit = parent.state.settings.blur_explicit_images && self.item.explicit();
        let key = parent.cover_cache_key_sized(&self.item, 0.);
        let source = parent
            .covers
            .get(&key)
            .filter(|path| path.exists())
            .cloned();
        div()
            .id("library-artwork-viewer")
            .debug_selector(|| "library-artwork-viewer".into())
            .relative()
            .w_full()
            .h((window.viewport_size().height - px(106. * zoom)).max(px(0.)))
            .overflow_hidden()
            .cursor_pointer()
            .when(source.is_none(), |viewer| {
                viewer.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_2xl()
                        .text_color(cx.theme().muted_foreground)
                        .child("♫"),
                )
            })
            .when_some(source, |viewer, source| {
                viewer.child(
                    img(source)
                        .debug_selector(|| "library-artwork-viewer-image".into())
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Contain)
                        .rounded(radius)
                        .when(explicit, |image| {
                            image
                                .foreground_filters(vec![BackdropFilterOperation::Blur(10. * zoom)])
                        }),
                )
            })
            .when(explicit, |viewer| {
                viewer.child(
                    div()
                        .absolute()
                        .inset_0()
                        .rounded(radius)
                        .debug_selector(|| "library-artwork-viewer-censor".into())
                        .bg(cx.theme().background.opacity(0.5)),
                )
            })
            .on_click(|_, window, cx| window.close_dialog(cx))
    }
}

impl PlaceholderCache {
    pub(crate) fn get(&mut self, item: &Item, priority: Placeholder) -> Option<Arc<RenderImage>> {
        if priority == Placeholder::Off {
            return None;
        }
        let key = format!(
            "{:x}",
            feishin_core::api::cover_hash(&format!(
                "{priority:?}:{}:{}:{}",
                item.thumb_hash, item.blur_hash, item.dominant_color
            ))
        );
        if let Some(image) = self.entries.get(&key) {
            return image.clone();
        }
        let image = images::decode(item, priority).and_then(|mut pixels| {
            // GPUI's image loader also converts straight RGBA into BGRA.
            for pixel in pixels.rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.rgba)
                .map(|image| Arc::new(RenderImage::new(vec![image::Frame::new(image)])))
        });
        if self.entries.len() >= 5000
            && let Some(old) = self.order.pop_front()
        {
            self.entries.remove(&old);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, image.clone());
        image
    }
}

impl Feishin {
    pub fn cover_cache_key(&self, item: &Item, placement: Placement) -> String {
        let size = self.cover_resolution(item, placement);
        self.cover_cache_key_sized(item, size)
    }

    pub(crate) fn cover_cache_key_sized(&self, item: &Item, size: f64) -> String {
        let key = images::cache_key(item, size);
        let version = self
            .cover_versions
            .get(&Self::cover_key(item))
            .copied()
            .unwrap_or(0);
        let key = if version == 0 {
            key
        } else {
            format!("{key}-revision-{version}")
        };
        feishin_core::cache::versioned(&key, self.state.cache_revision)
    }

    fn cover_resolution(&self, item: &Item, placement: Placement) -> f64 {
        if item.kind == ItemKind::Radio
            && self
                .sessions
                .get(&item.server_id)
                .is_some_and(|api| api.server.kind == ServerType::Jellyfin)
        {
            400. // Jellyfin stations keep user-selected artwork locally.
        } else {
            self.state.settings.image_resolution.get(placement)
        }
    }

    pub fn artwork_path_at(&self, item: &Item, placement: Placement) -> Option<&PathBuf> {
        let covers = if self.state.settings.blur_explicit_images && item.explicit() {
            &self.censored_covers
        } else {
            &self.covers
        };
        covers
            .get(&self.cover_cache_key(item, placement))
            .filter(|path| path.exists())
            .or_else(|| {
                covers
                    .get(&Self::cover_key(item))
                    .filter(|path| path.exists())
            })
    }

    #[cfg(test)]
    pub fn artwork_path(&self, item: &Item) -> Option<&PathBuf> {
        self.artwork_path_at(item, Placement::Table)
    }

    pub(crate) fn image_content(&self, item: &Item, placement: Placement, cx: &App) -> AnyElement {
        self.image_content_with_radius(item, placement, px(0.), cx)
    }

    pub(crate) fn image_content_with_radius(
        &self,
        item: &Item,
        placement: Placement,
        radius: Pixels,
        cx: &App,
    ) -> AnyElement {
        self.image_content_in(item, placement, radius, None, cx)
    }

    pub(crate) fn image_content_in(
        &self,
        item: &Item,
        placement: Placement,
        radius: Pixels,
        styles: Option<(
            feishin_core::styles::Target,
            Vec<feishin_core::styles::Target>,
        )>,
        cx: &App,
    ) -> AnyElement {
        self.image_content_in_fit(item, placement, radius, styles, false, cx)
    }

    pub(crate) fn image_content_with_fit(
        &self,
        item: &Item,
        placement: Placement,
        force_cover: bool,
        cx: &App,
    ) -> AnyElement {
        let image = feishin_core::styles::Target::new("img", "", &["fs-image-module-image"]);
        let cell = feishin_core::styles::Target::new("div", "", &["ag-cell"]);
        self.image_content_in_fit(
            item,
            placement,
            px(0.),
            Some((image, vec![cell])),
            force_cover,
            cx,
        )
    }

    fn image_content_in_fit(
        &self,
        item: &Item,
        placement: Placement,
        radius: Pixels,
        styles: Option<(
            feishin_core::styles::Target,
            Vec<feishin_core::styles::Target>,
        )>,
        force_cover: bool,
        cx: &App,
    ) -> AnyElement {
        let fit = || {
            if force_cover {
                ObjectFit::Cover
            } else if self.state.settings.native_image_aspect_ratio
                || (placement == Placement::FullScreenPlayer
                    && self.state.settings.player_display.use_image_aspect_ratio)
            {
                ObjectFit::Contain
            } else {
                ObjectFit::Cover
            }
        };
        let placeholder = self
            .image_placeholders
            .borrow_mut()
            .get(item, self.state.settings.image_placeholder);
        let mut content = div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .justify_center();
        if let Some(image) = placeholder {
            content = content.child(
                div()
                    .debug_selector(|| "artwork-placeholder".into())
                    .absolute()
                    .inset_0()
                    .child(
                        img(image)
                            .size_full()
                            .rounded(radius)
                            .object_fit(fit())
                            .map(|image| {
                                if let Some((target, parents)) = &styles {
                                    crate::styles::apply_in(
                                        image,
                                        target.clone(),
                                        parents.clone(),
                                        cx,
                                    )
                                } else {
                                    image
                                }
                            }),
                    ),
            );
        } else {
            content = content.child(
                div()
                    .text_2xl()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.tr("♫")),
            );
        }
        if let Some(path) = self.artwork_path_at(item, placement) {
            content = content.child(
                img(path.clone())
                    .absolute()
                    .inset_0()
                    .size_full()
                    .rounded(radius)
                    .object_fit(fit())
                    .map(|image| {
                        if let Some((target, parents)) = &styles {
                            crate::styles::apply_in(image, target.clone(), parents.clone(), cx)
                        } else {
                            image
                        }
                    }),
            );
        }
        content.into_any_element()
    }

    pub(crate) fn artwork_at(
        &self,
        item: &Item,
        size: f32,
        placement: Placement,
        cx: &App,
    ) -> AnyElement {
        div()
            .size(px(size))
            .flex_shrink_0()
            .rounded_md()
            .overflow_hidden()
            .bg(cx.theme().secondary)
            .flex()
            .items_center()
            .justify_center()
            .child(self.image_content(item, placement, cx))
            .when(true, |div| {
                crate::styles::apply(
                    div,
                    "img",
                    "",
                    &["fs-image-module-image"],
                    (placement == Placement::Table)
                        .then(|| feishin_core::styles::Target::new("div", "", &["ag-cell"])),
                    cx,
                )
            })
            .into_any_element()
    }

    pub fn detail_artwork(&self, item: &Item, size: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let click = item.clone();
        let keyboard = item.clone();
        div()
            .id("library-detail-artwork")
            .group("library-detail-artwork")
            .relative()
            .size(size)
            .flex_shrink_0()
            .debug_selector(|| "library-detail-artwork".into())
            .role(Role::Button)
            .key_context("FormControl")
            .aria_label(format!("{} {}", self.tr("common.view"), item.name))
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .child(
                div()
                    .size_full()
                    .rounded(px(5. * self.state.settings.zoom))
                    .bg(cx.theme().secondary)
                    .foreground_filters(vec![BackdropFilterOperation::DropShadow {
                        offset: [0., 0.],
                        sigma: 8. * self.state.settings.zoom,
                        color: Some([0., 0., 0., 0.5]),
                    }])
                    .child(self.image_content_with_radius(
                        item,
                        Placement::Header,
                        px(5. * self.state.settings.zoom),
                        cx,
                    ))
                    .map(|image| {
                        crate::styles::apply(image, "img", "", &["fs-image-module-image"], None, cx)
                    }),
            )
            .when(self.can_upload_library_image(item), |artwork| {
                let item = item.clone();
                artwork
                    .child(
                        div()
                            .absolute()
                            .inset(-cx.theme().font_size * 0.5)
                            .rounded(cx.theme().font_size * 0.3125)
                            .border(px(3. * self.state.settings.zoom))
                            .border_color(cx.theme().primary)
                            .invisible()
                            .group_drag_over::<ExternalPaths>("library-detail-artwork", |style| {
                                style.visible()
                            }),
                    )
                    .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                        if let Some(path) = first_image_path(paths.paths()) {
                            this.change_library_image(item.clone(), Some(path.clone()), cx);
                        }
                        cx.stop_propagation();
                    }))
            })
            .on_click(
                cx.listener(move |this, _, window, cx| {
                    this.open_artwork(click.clone(), window, cx)
                }),
            )
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    this.open_artwork(keyboard.clone(), window, cx);
                }
            }))
            .into_any_element()
    }

    pub fn open_artwork(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        if item.cover_id.is_empty()
            || !matches!(
                item.kind,
                ItemKind::Album
                    | ItemKind::Artist
                    | ItemKind::AlbumArtist
                    | ItemKind::Playlist
                    | ItemKind::Song
            )
        {
            return;
        }
        self.fetch_item_cover_sized(&item, 0., None, cx);
        let parent = cx.entity();
        let content = cx.new(|cx| ArtworkDialog {
            parent: parent.clone(),
            item,
            _subscription: cx.observe(&parent, |_, _, cx| cx.notify()),
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            let zoom = cx
                .try_global::<crate::styles::NativeStyles>()
                .map_or(1., |styles| styles.zoom());
            let font = cx.theme().font_size;
            dialog
                .width(window.viewport_size().width)
                .h(window.viewport_size().height)
                .margin_top(px(0.))
                .fade_only(true)
                .rounded_none()
                .border(px(2. * zoom))
                .p(font * 0.75)
                .pt_0()
                .gap_0()
                .close_button(false)
                .title(
                    div()
                        .h_flex()
                        .items_center()
                        .justify_end()
                        .h(font * 3.75)
                        .child(
                            crate::styles::button("library-artwork-viewer-close", cx)
                                .debug_selector(|| "library-artwork-viewer-close".into())
                                .ghost()
                                .size(font * 1.75)
                                .p_0()
                                .aria_label(crate::appearance::text("common.close", cx))
                                .icon(Icon::new(IconName::Close).with_size(font * 1.5))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        ),
                )
                .child(content.clone())
        });
    }

    pub fn fetch_covers(&mut self, cx: &mut Context<Self>) {
        let placement = if self.list_config().display != "poster" {
            Placement::Table
        } else {
            Placement::ItemCard
        };
        for item in self.items.clone() {
            self.fetch_item_cover_at(&item, placement, cx);
        }
        if let Route::Detail(item) = self.route.clone() {
            self.fetch_item_cover_at(&item, Placement::Header, cx);
            if matches!(
                item.kind,
                ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
            ) {
                self.fetch_item_cover_at(&item, Placement::ItemCard, cx);
            }
        }
        for item in self.info_items.clone() {
            self.fetch_item_cover(&item, cx);
        }
        let sections = self
            .artist_sections
            .iter()
            .flat_map(|(id, section)| {
                section.items.iter().take(24).cloned().map(move |item| {
                    (
                        item,
                        if id == "recentAlbums" || id == "similarArtists" || id == "compilations" {
                            Placement::ItemCard
                        } else {
                            Placement::Table
                        },
                    )
                })
            })
            .collect::<Vec<_>>();
        for (item, placement) in sections {
            self.fetch_item_cover_at(&item, placement, cx);
        }
        for item in self.playlist_options.clone() {
            self.fetch_item_cover(&item, cx);
        }
        let home = self
            .home_sections
            .values()
            .flat_map(|section| section.items.iter().cloned())
            .collect::<Vec<_>>();
        for item in home {
            self.fetch_item_cover_at(&item, Placement::ItemCard, cx);
        }
        let related = self
            .album_related
            .iter()
            .flat_map(|section| section.state.items.iter().cloned())
            .collect::<Vec<_>>();
        for item in related {
            self.fetch_item_cover_at(&item, Placement::ItemCard, cx);
        }
        if let Some(item) = self.current_entry().map(|entry| entry.item.clone()) {
            self.fetch_item_cover_at(&item, Placement::Table, cx);
            if self.state.settings.sidebar.image_enabled
                && self.state.settings.sidebar.image_shown
                && !self.state.settings.sidebar.collapsed
            {
                self.fetch_item_cover_at(&item, Placement::Sidebar, cx);
            }
            if self.full_player {
                self.fetch_item_cover_at(&item, Placement::FullScreenPlayer, cx);
                self.fetch_item_cover_at(&item, Placement::ItemCard, cx);
            }
        }
        if self.full_player {
            let items = [
                self.state.queue.current(),
                self.state.queue.next_entry(true),
            ]
            .into_iter()
            .flatten()
            .map(|entry| entry.item.clone())
            .collect::<Vec<_>>();
            for item in items {
                self.fetch_item_cover_at(&item, Placement::ItemCard, cx);
                if let Some(album) = crate::player_display::background_album_item(&item) {
                    self.fetch_item_cover_at(&album, Placement::ItemCard, cx);
                }
            }
        }
    }

    pub fn fetch_item_cover(&mut self, item: &Item, cx: &mut Context<Self>) {
        self.fetch_item_cover_at(item, Placement::Table, cx);
    }

    pub fn fetch_item_cover_at(
        &mut self,
        item: &Item,
        placement: Placement,
        cx: &mut Context<Self>,
    ) {
        self.fetch_item_cover_sized(
            item,
            self.cover_resolution(item, placement),
            Some(placement),
            cx,
        );
    }

    fn fetch_item_cover_sized(
        &mut self,
        item: &Item,
        size: f64,
        placement: Option<Placement>,
        cx: &mut Context<Self>,
    ) {
        if self.cache_busy {
            return;
        }
        if item.cover_id.is_empty() && item.kind != ItemKind::Radio {
            return;
        }
        let key = self.cover_cache_key_sized(item, size);
        if self.covers.contains_key(&key) {
            if let Some(placement) = placement {
                self.prepare_censored_cover_at(item, placement, cx);
            }
            return;
        }
        let path = self.data_dir.join("covers").join(format!("{key}.img"));
        self.covers.insert(key.clone(), path.clone());
        if path.exists() {
            if let Some(placement) = placement {
                self.prepare_censored_cover_at(item, placement, cx);
            }
            return;
        }
        let Some(api) = self.sessions.get(&item.server_id).cloned() else {
            return;
        };
        let local = item.kind == ItemKind::Radio && api.server.kind == ServerType::Jellyfin;
        let local_source = self
            .data_dir
            .join("covers")
            .join(format!("{}.img", Self::cover_key(item)));
        if local && local_source == path {
            return;
        }
        let item = item.clone();
        let downloaded = item.clone();
        let revision = self.state.cache_revision;
        let stale_path = path.clone();
        let task = cx.background_executor().spawn(async move {
            if local {
                std::fs::copy(local_source, path)
                    .map(|_| ())
                    .map_err(anyhow::Error::from)
            } else {
                api.cover_sized(&item, &path, size)
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.state.cache_revision != revision {
                    feishin_core::cache::discard_stale(&stale_path);
                    return;
                }
                if let Err(error) = result {
                    log::debug!("Cover download failed: {error}");
                }
                if let Some(placement) = placement
                    && this.cover_cache_key(&downloaded, placement) == key
                {
                    this.prepare_censored_cover_at(&downloaded, placement, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn invalidate_artwork(&mut self, base: &str, keep_local: bool) {
        *self.cover_versions.entry(base.into()).or_default() += 1;
        let matches = |key: &str| key == base || key.starts_with(&format!("{base}-"));
        self.covers.retain(|key, _| !matches(key));
        self.censored_covers.retain(|key, _| !matches(key));
        for (directory, extension) in [("covers", "img"), ("censored-covers", "png")] {
            if let Ok(files) = std::fs::read_dir(self.data_dir.join(directory)) {
                for file in files.flatten() {
                    let path = file.path();
                    if path
                        .file_stem()
                        .is_some_and(|stem| matches(&stem.to_string_lossy()))
                        && path.extension().is_some_and(|ext| ext == extension)
                        && !(keep_local
                            && directory == "covers"
                            && path.file_stem().is_some_and(|stem| stem == base))
                        && let Err(error) = std::fs::remove_file(path)
                    {
                        log::debug!("Could not remove old artwork cache: {error}");
                    }
                }
            }
        }
        self.player_background = None;
        self.player_background_key.clear();
        self.detail_background = None;
        self.detail_background_key.clear();
        self.home_backgrounds.clear();
    }

    pub fn sync_image_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (placement, input) in Placement::ALL
            .into_iter()
            .zip(&self.image_resolution_inputs)
        {
            input.update(cx, |input, cx| {
                input.set_value(
                    self.state
                        .settings
                        .image_resolution
                        .get(placement)
                        .to_string(),
                    window,
                    cx,
                )
            });
        }
    }

    pub fn apply_image_resolution(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(size) = self.image_resolution_inputs[index]
            .read(cx)
            .value()
            .parse::<f64>()
            .ok()
            .filter(|size| size.is_finite())
        else {
            return;
        };
        let placement = Placement::ALL[index];
        if self.state.settings.image_resolution.get(placement) == size {
            return;
        }
        self.state.settings.image_resolution.set(placement, size);
        self.player_background_key.clear();
        self.detail_background_key.clear();
        self.fetch_covers(cx);
        self.save();
        cx.notify();
    }

    pub fn image_settings_rows(&self, query: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let matches = |key| {
            feishin_core::settings_search::matches(&self.state.settings.language, query, &[key])
        };
        let mut rows = Vec::new();
        if matches("setting.imageAspectRatio") {
            rows.push(crate::settings::option_row(
                "native-image-aspect",
                "setting.imageAspectRatio",
                gpui_component::switch::Switch::new("native-image-aspect")
                    .checked(self.state.settings.native_image_aspect_ratio)
                    .tooltip(self.tr("setting.imageAspectRatio"))
                    .settings(
                        self.state.settings.zoom,
                        crate::appearance::rtl(cx),
                        "native-image-aspect",
                    )
                    .on_click(cx.listener(|this, value: &bool, _, cx| {
                        this.state.settings.native_image_aspect_ratio = *value;
                        this.save();
                        cx.notify();
                    }))
                    .into_any_element(),
                self,
                cx,
            ));
        }
        if matches("setting.imagePlaceholderPriority") {
            rows.push(crate::settings::option_row(
                "placeholder-priority",
                "setting.imagePlaceholderPriority",
                crate::styles::button("placeholder-priority", cx)
                    .aria_label(self.tr("setting.imagePlaceholderPriority"))
                    .label(self.tr(self.state.settings.image_placeholder.label_key()))
                    .dropdown_menu({
                        let view = cx.entity().downgrade();
                        move |mut menu, _, cx| {
                            for value in Placeholder::ALL {
                                let view = view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(crate::appearance::text(
                                        value.label_key(),
                                        cx,
                                    ))
                                    .on_click(
                                        move |_, _, cx| {
                                            _ = view.update(cx, |this, cx| {
                                                this.state.settings.image_placeholder = value;
                                                this.save();
                                                cx.notify();
                                            });
                                        },
                                    ),
                                );
                            }
                            menu
                        }
                    })
                    .into_any_element(),
                self,
                cx,
            ));
        }
        if matches("setting.imageResolution") {
            rows.push(crate::settings::option_row(
                "image-resolution",
                "setting.imageResolution",
                crate::styles::button("image-resolution-toggle", cx)
                    .debug_selector(|| "image-resolution-toggle".into())
                    .when(!self.image_resolution_open, |button| button.primary())
                    .when(self.image_resolution_open, |button| button.ghost())
                    .label(self.tr(if self.image_resolution_open {
                        "common.close"
                    } else {
                        "common.edit"
                    }))
                    .h(rems(1.875))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.image_resolution_open {
                            window.blur();
                        }
                        this.image_resolution_open = !this.image_resolution_open;
                        if this.image_resolution_open {
                            this.sync_image_inputs(window, cx);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
                self,
                cx,
            ));
            if self.image_resolution_open {
                rows.push(
                    div()
                        .v_flex()
                        .w_full()
                        .children([4, 2, 3, 1, 0].map(|index| {
                            let placement = Placement::ALL[index];
                            div()
                                .h_flex()
                                .justify_between()
                                .items_center()
                                .w_full()
                                .py(rems(0.4375))
                                .px(rems(0.625))
                                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                                .child(self.tr(placement.label_key()))
                                .child(
                                    crate::styles::number_input(
                                        &self.image_resolution_inputs[index],
                                        cx,
                                    )
                                    .hide_controls(true)
                                    .suffix(
                                        div()
                                            .pr(rems(0.75))
                                            .text_color(cx.theme().muted_foreground)
                                            .child("px"),
                                    )
                                    .w(px(90. * self.state.settings.zoom)),
                                )
                        }))
                        .into_any_element(),
                );
            }
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::{Arc, Item, Placeholder, PlaceholderCache};

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn inline_image_resolution_matches_original_input_states(cx: &mut gpui::TestAppContext) {
        use crate::app::{Feishin, Route};
        use gpui::{AppContext, Modifiers, px, size};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-image-settings-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let capture = captured.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let mut state = feishin_core::storage::State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.lyric_fetch = false;
            state.settings.scrobble = false;
            state.settings.mpv_path = directory.join("missing-mpv").to_string_lossy().into();
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.route = Route::Settings;
                view.settings_search.update(cx, |input, cx| {
                    input.set_value("Image resolution", window, cx)
                });
            });
            capture.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1100.), px(900.)));
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/image-resolution-reference.json"))
                .unwrap();
        for case in &cases {
            let name = case["name"].as_str().unwrap();
            match name {
                "open" | "close" | "reopen" => {
                    let bounds = cx.debug_bounds("image-resolution-toggle").unwrap();
                    cx.simulate_click(bounds.center(), Modifiers::none());
                }
                "increment" => cx.simulate_keystrokes("up"),
                "decrement" => cx.simulate_keystrokes("down"),
                "clamp-maximum" | "clamp-minimum" | "blur-empty" | "blur-decimal" => {
                    cx.update(|window, _| window.blur())
                }
                "closed" => {}
                _ => {
                    let (index, text) = match name {
                        "table" => (4, "123"),
                        "original" => (2, "0"),
                        "maximum" => (3, "2000"),
                        "over-maximum" => (3, "2001"),
                        "negative" => (1, "-1"),
                        "empty" => (0, ""),
                        "decimal" => (4, "123.5"),
                        _ => unreachable!("{name}"),
                    };
                    let input =
                        app.read_with(cx, |view, _| view.image_resolution_inputs[index].clone());
                    cx.update(|window, cx| input.update(cx, |input, cx| input.focus(window, cx)));
                    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
                        "cmd-a"
                    } else {
                        "ctrl-a"
                    });
                    if text.is_empty() {
                        cx.simulate_keystrokes("backspace");
                    } else {
                        cx.simulate_input(text);
                    }
                }
            }
            cx.run_until_parked();
            let expected: feishin_core::images::Resolution =
                serde_json::from_value(case["stored"].clone()).unwrap();
            app.read_with(cx, |view, cx| {
                assert_eq!(view.route, Route::Settings);
                assert_eq!(view.state.settings.image_resolution, expected, "{name}");
                assert_eq!(
                    view.image_resolution_open,
                    !matches!(name, "closed" | "close")
                );
                for (index, row) in [4, 2, 3, 1, 0]
                    .into_iter()
                    .zip(case["rows"].as_array().unwrap())
                {
                    assert_eq!(
                        view.image_resolution_inputs[index]
                            .read(cx)
                            .value()
                            .as_str(),
                        row["input"].as_str().unwrap(),
                        "{name}"
                    );
                }
            });
            if !matches!(name, "closed" | "open") {
                assert_eq!(
                    feishin_core::storage::load(&directory.join("state.json"))
                        .unwrap()
                        .settings
                        .image_resolution,
                    expected,
                    "{name} was not saved immediately"
                );
            }
        }
        assert_eq!(
            app.read_with(cx, |view, cx| view.image_resolution_inputs[0]
                .read(cx)
                .value()
                .to_string()),
            "0",
            "Reopening resets an empty draft to the saved value"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn placeholders_use_bgra_and_bounded_fifo_without_redecoding() {
        let mut cache = PlaceholderCache::default();
        let mut item = Item {
            dominant_color: "#10203080".into(),
            ..Default::default()
        };
        let first = cache.get(&item, Placeholder::DominantColor).unwrap();
        assert_eq!(first.as_bytes(0).unwrap(), [48, 32, 16, 128]);
        assert!(Arc::ptr_eq(
            &first,
            &cache.get(&item, Placeholder::DominantColor).unwrap()
        ));
        assert!(cache.get(&item, Placeholder::Off).is_none());
        assert_eq!(cache.entries.len(), 1);
        for i in 0..5000 {
            item.dominant_color = format!("#{i:06x}");
            cache.get(&item, Placeholder::DominantColor);
        }
        assert_eq!(cache.entries.len(), 5000);
        assert_eq!(cache.order.len(), 5000);
        item.dominant_color = "#10203080".into();
        assert!(!Arc::ptr_eq(
            &first,
            &cache.get(&item, Placeholder::DominantColor).unwrap()
        ));
    }
}
