use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct Assets;

#[cfg(target_os = "macos")]
fn macos_app_icon() -> anyhow::Result<objc2::rc::Retained<objc2_app_kit::NSImage>> {
    use anyhow::Context;
    use objc2::AnyThread;
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSData;

    let data = NSData::with_bytes(include_bytes!("../../../assets/icons/1024x1024.png"));
    NSImage::initWithData(NSImage::alloc(), &data).context("Cannot decode the Feishin app icon")
}

#[cfg(target_os = "macos")]
pub fn set_macos_app_icon() -> anyhow::Result<()> {
    use anyhow::Context;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let marker = MainThreadMarker::new().context("The app icon must be set on the main thread")?;
    let icon = macos_app_icon()?;
    // Direct Cargo launches have no bundle icon; also refresh stale development bundle icons.
    // SAFETY: AppKit is called on the main thread with a non-nil, retained image.
    unsafe { NSApplication::sharedApplication(marker).setApplicationIconImage(Some(&icon)) };
    Ok(())
}

pub const RATING: [&str; 2] = ["rating-outline", "rating-fill"];
pub const PLAYER: [&str; 10] = [
    "previous",
    "next",
    "play",
    "pause",
    "stop",
    "shuffle",
    "step-back",
    "step-forward",
    "repeat",
    "repeat-one",
];
pub const INTERFACE: [&str; 100] = [
    "file-json",
    "info",
    "disc",
    "hash",
    "drag-vertical",
    "arrow-up",
    "arrow-left-right",
    "filter",
    "layout-grid",
    "layout-detail",
    "layout-table",
    "sort-ascending",
    "sort-descending",
    "window-minimize",
    "window-maximize",
    "window-close",
    "album",
    "song",
    "duration",
    "user",
    "artist",
    "folder",
    "folder-closed",
    "folders-expand",
    "folders-collapse",
    "playlist",
    "genre",
    "radio",
    "library",
    "keyboard",
    "more",
    "close",
    "play-next",
    "play-last",
    "random",
    "spinner",
    "arrow-left",
    "arrow-right",
    "upload",
    "download",
    "delete",
    "go-current",
    "playlist-add",
    "settings",
    "group",
    "star",
    "circle-slash",
    "search",
    "menu",
    "collection",
    "expand",
    "shrink",
    "visualizer",
    "microphone",
    "queue",
    "related",
    "sliders",
    "cast",
    "timer",
    "timer-off",
    "panel-open",
    "panel-close",
    "volume-low",
    "volume-high",
    "volume-muted",
    "favorite",
    "favorite-filled",
    "arrow-down",
    "arrow-forward",
    "error",
    "wifi-off",
    "refresh",
    "metadata",
    "languages",
    "minus",
    "plus",
    "RiAlbumFill",
    "RiAlbumLine",
    "RiFlag2Fill",
    "RiFlag2Line",
    "RiFolder3Fill",
    "RiFolder3Line",
    "RiHeartFill",
    "RiHeartLine",
    "RiHome6Fill",
    "RiHome6Line",
    "RiMusic2Fill",
    "RiMusic2Line",
    "RiPlayFill",
    "RiPlayLine",
    "RiPlayListFill",
    "RiPlayListLine",
    "RiRadioFill",
    "RiRadioLine",
    "RiSearchFill",
    "RiSearchLine",
    "RiSettings2Fill",
    "RiSettings2Line",
    "RiUserVoiceFill",
    "RiUserVoiceLine",
];
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "native-icons/info.svg" => Some(include_bytes!("../assets/icons/info.svg")),
            "native-icons/disc.svg" => Some(include_bytes!("../assets/icons/disc.svg")),
            "native-icons/hash.svg" => Some(include_bytes!("../assets/icons/hash.svg")),
            "native-icons/drag-vertical.svg" => {
                Some(include_bytes!("../assets/icons/drag-vertical.svg"))
            }
            "native-icons/filter.svg" => Some(include_bytes!("../assets/icons/filter.svg")),
            "native-icons/layout-grid.svg" => {
                Some(include_bytes!("../assets/icons/layout-grid.svg"))
            }
            "native-icons/layout-detail.svg" => {
                Some(include_bytes!("../assets/icons/layout-detail.svg"))
            }
            "native-icons/layout-table.svg" => {
                Some(include_bytes!("../assets/icons/layout-table.svg"))
            }
            "native-icons/sort-ascending.svg" => {
                Some(include_bytes!("../assets/icons/sort-ascending.svg"))
            }
            "native-icons/sort-descending.svg" => {
                Some(include_bytes!("../assets/icons/sort-descending.svg"))
            }
            "native-icons/window-minimize.svg" => {
                Some(include_bytes!("../assets/icons/window-minimize.svg"))
            }
            "native-icons/window-maximize.svg" => {
                Some(include_bytes!("../assets/icons/window-maximize.svg"))
            }
            "native-icons/window-close.svg" => {
                Some(include_bytes!("../assets/icons/window-close.svg"))
            }
            "native-window/app.png" => Some(include_bytes!("../../../assets/icons/32x32.png")),
            "native-window/close.png" => {
                Some(include_bytes!("../assets/window-controls/close-mac.png"))
            }
            "native-window/close-hover.png" => Some(include_bytes!(
                "../assets/window-controls/close-mac-hover.png"
            )),
            "native-window/min.png" => {
                Some(include_bytes!("../assets/window-controls/min-mac.png"))
            }
            "native-window/min-hover.png" => Some(include_bytes!(
                "../assets/window-controls/min-mac-hover.png"
            )),
            "native-window/max.png" => {
                Some(include_bytes!("../assets/window-controls/max-mac.png"))
            }
            "native-window/max-hover.png" => Some(include_bytes!(
                "../assets/window-controls/max-mac-hover.png"
            )),
            "native-icons/metadata.svg" => Some(include_bytes!("../assets/icons/metadata.svg")),
            "native-icons/languages.svg" => Some(include_bytes!("../assets/icons/languages.svg")),
            "native-icons/minus.svg" => Some(include_bytes!("../assets/icons/minus.svg")),
            "native-icons/plus.svg" => Some(include_bytes!("../assets/icons/plus.svg")),
            "native-icons/group.svg" => Some(include_bytes!("../assets/icons/group.svg")),
            "native-icons/star.svg" => Some(include_bytes!("../assets/icons/star.svg")),
            "native-icons/circle-slash.svg" => {
                Some(include_bytes!("../assets/icons/circle-slash.svg"))
            }
            "native-icons/timer.svg" => Some(include_bytes!("../assets/icons/timer.svg")),
            "native-icons/timer-off.svg" => Some(include_bytes!("../assets/icons/timer-off.svg")),
            "native-icons/panel-open.svg" => Some(include_bytes!("../assets/icons/panel-open.svg")),
            "native-icons/panel-close.svg" => {
                Some(include_bytes!("../assets/icons/panel-close.svg"))
            }

            "provider-logos/lastfm.png" => Some(include_bytes!(
                "../assets/service-icons/lastfm_logo_icon.png"
            )),
            "provider-logos/listenbrainz.svg" => Some(include_bytes!(
                "../assets/service-icons/listenbrainz_logo_icon.svg"
            )),
            "provider-logos/musicbrainz.svg" => Some(include_bytes!(
                "../assets/service-icons/musicbrainz_logo_icon.svg"
            )),
            "provider-logos/qobuz.png" => Some(include_bytes!(
                "../assets/service-icons/qobuz_logo_icon.png"
            )),
            "provider-logos/spotify.svg" => Some(include_bytes!(
                "../assets/service-icons/spotify_logo_icon.svg"
            )),
            "native-icons/previous.svg" => Some(include_bytes!("../assets/icons/previous.svg")),
            "native-icons/next.svg" => Some(include_bytes!("../assets/icons/next.svg")),
            "native-icons/spinner.svg" => Some(include_bytes!("../assets/icons/spinner.svg")),
            "native-icons/RiAlbumFill.svg" => {
                Some(include_bytes!("../assets/icons/RiAlbumFill.svg"))
            }
            "native-icons/RiAlbumLine.svg" => {
                Some(include_bytes!("../assets/icons/RiAlbumLine.svg"))
            }
            "native-icons/RiFlag2Fill.svg" => {
                Some(include_bytes!("../assets/icons/RiFlag2Fill.svg"))
            }
            "native-icons/RiFlag2Line.svg" => {
                Some(include_bytes!("../assets/icons/RiFlag2Line.svg"))
            }
            "native-icons/RiFolder3Fill.svg" => {
                Some(include_bytes!("../assets/icons/RiFolder3Fill.svg"))
            }
            "native-icons/RiFolder3Line.svg" => {
                Some(include_bytes!("../assets/icons/RiFolder3Line.svg"))
            }
            "native-icons/RiHeartFill.svg" => {
                Some(include_bytes!("../assets/icons/RiHeartFill.svg"))
            }
            "native-icons/RiHeartLine.svg" => {
                Some(include_bytes!("../assets/icons/RiHeartLine.svg"))
            }
            "native-icons/RiHome6Fill.svg" => {
                Some(include_bytes!("../assets/icons/RiHome6Fill.svg"))
            }
            "native-icons/RiHome6Line.svg" => {
                Some(include_bytes!("../assets/icons/RiHome6Line.svg"))
            }
            "native-icons/RiMusic2Fill.svg" => {
                Some(include_bytes!("../assets/icons/RiMusic2Fill.svg"))
            }
            "native-icons/RiMusic2Line.svg" => {
                Some(include_bytes!("../assets/icons/RiMusic2Line.svg"))
            }
            "native-icons/RiPlayFill.svg" => Some(include_bytes!("../assets/icons/RiPlayFill.svg")),
            "native-icons/RiPlayLine.svg" => Some(include_bytes!("../assets/icons/RiPlayLine.svg")),
            "native-icons/RiPlayListFill.svg" => {
                Some(include_bytes!("../assets/icons/RiPlayListFill.svg"))
            }
            "native-icons/RiPlayListLine.svg" => {
                Some(include_bytes!("../assets/icons/RiPlayListLine.svg"))
            }
            "native-icons/RiRadioFill.svg" => {
                Some(include_bytes!("../assets/icons/RiRadioFill.svg"))
            }
            "native-icons/RiRadioLine.svg" => {
                Some(include_bytes!("../assets/icons/RiRadioLine.svg"))
            }
            "native-icons/RiSearchFill.svg" => {
                Some(include_bytes!("../assets/icons/RiSearchFill.svg"))
            }
            "native-icons/RiSearchLine.svg" => {
                Some(include_bytes!("../assets/icons/RiSearchLine.svg"))
            }
            "native-icons/RiSettings2Fill.svg" => {
                Some(include_bytes!("../assets/icons/RiSettings2Fill.svg"))
            }
            "native-icons/RiSettings2Line.svg" => {
                Some(include_bytes!("../assets/icons/RiSettings2Line.svg"))
            }
            "native-icons/RiUserVoiceFill.svg" => {
                Some(include_bytes!("../assets/icons/RiUserVoiceFill.svg"))
            }
            "native-icons/RiUserVoiceLine.svg" => {
                Some(include_bytes!("../assets/icons/RiUserVoiceLine.svg"))
            }

            "native-icons/arrow-left.svg" => Some(include_bytes!("../assets/icons/arrow-left.svg")),
            "native-icons/arrow-right.svg" => {
                Some(include_bytes!("../assets/icons/arrow-right.svg"))
            }
            "native-icons/play.svg" => Some(include_bytes!("../assets/icons/play.svg")),
            "native-icons/error.svg" => Some(include_bytes!("../assets/icons/error.svg")),
            "native-icons/wifi-off.svg" => Some(include_bytes!("../assets/icons/wifi-off.svg")),
            "native-icons/refresh.svg" => Some(include_bytes!("../assets/icons/refresh.svg")),
            "native-icons/file-json.svg" => Some(include_bytes!("../assets/icons/file-json.svg")),
            "native-icons/pause.svg" => Some(include_bytes!("../assets/icons/pause.svg")),
            "native-icons/stop.svg" => Some(include_bytes!("../assets/icons/stop.svg")),
            "native-icons/shuffle.svg" => Some(include_bytes!("../assets/icons/shuffle.svg")),
            "native-icons/step-back.svg" => Some(include_bytes!("../assets/icons/step-back.svg")),
            "native-icons/step-forward.svg" => {
                Some(include_bytes!("../assets/icons/step-forward.svg"))
            }
            "native-icons/repeat.svg" => Some(include_bytes!("../assets/icons/repeat.svg")),
            "native-icons/repeat-one.svg" => Some(include_bytes!("../assets/icons/repeat-one.svg")),
            "native-icons/album.svg" => Some(include_bytes!("../assets/icons/album.svg")),
            "native-icons/duration.svg" => Some(include_bytes!("../assets/icons/duration.svg")),
            "native-icons/user.svg" => Some(include_bytes!("../assets/icons/user.svg")),
            "native-icons/song.svg" => Some(include_bytes!("../assets/icons/song.svg")),
            "native-icons/artist.svg" => Some(include_bytes!("../assets/icons/artist.svg")),
            "native-icons/folder.svg" => Some(include_bytes!("../assets/icons/folder.svg")),
            "native-icons/folder-closed.svg" => {
                Some(include_bytes!("../assets/icons/folder-closed.svg"))
            }
            "native-icons/folders-expand.svg" => {
                Some(include_bytes!("../assets/icons/folders-expand.svg"))
            }
            "native-icons/folders-collapse.svg" => {
                Some(include_bytes!("../assets/icons/folders-collapse.svg"))
            }
            "native-icons/playlist.svg" => Some(include_bytes!("../assets/icons/playlist.svg")),
            "native-icons/genre.svg" => Some(include_bytes!("../assets/icons/genre.svg")),
            "native-icons/radio.svg" => Some(include_bytes!("../assets/icons/radio.svg")),
            "native-icons/library.svg" => Some(include_bytes!("../assets/icons/library.svg")),
            "native-icons/keyboard.svg" => Some(include_bytes!("../assets/icons/keyboard.svg")),
            "native-icons/favorite.svg" => Some(include_bytes!("../assets/icons/favorite.svg")),
            "native-icons/favorite-filled.svg" => {
                Some(include_bytes!("../assets/icons/favorite-filled.svg"))
            }
            "native-icons/arrow-forward.svg" => {
                Some(include_bytes!("../assets/icons/arrow-forward.svg"))
            }
            "native-icons/arrow-down.svg" => Some(include_bytes!("../assets/icons/arrow-down.svg")),
            "native-icons/more.svg" => Some(include_bytes!("../assets/icons/more.svg")),
            "native-icons/arrow-up.svg" => Some(include_bytes!("../assets/icons/arrow-up.svg")),
            "native-icons/arrow-left-right.svg" => {
                Some(include_bytes!("../assets/icons/arrow-left-right.svg"))
            }
            "native-icons/cast.svg" => Some(include_bytes!("../assets/icons/cast.svg")),
            "native-icons/close.svg" => Some(include_bytes!("../assets/icons/close.svg")),
            "native-icons/play-next.svg" => Some(include_bytes!("../assets/icons/play-next.svg")),
            "native-icons/play-last.svg" => Some(include_bytes!("../assets/icons/play-last.svg")),
            "native-icons/random.svg" => Some(include_bytes!("../assets/icons/random.svg")),
            "native-icons/upload.svg" => Some(include_bytes!("../assets/icons/upload.svg")),
            "native-icons/download.svg" => Some(include_bytes!("../assets/icons/download.svg")),
            "native-icons/delete.svg" => Some(include_bytes!("../assets/icons/delete.svg")),
            "native-icons/go-current.svg" => Some(include_bytes!("../assets/icons/go-current.svg")),
            "native-icons/playlist-add.svg" => {
                Some(include_bytes!("../assets/icons/playlist-add.svg"))
            }
            "native-icons/settings.svg" => Some(include_bytes!("../assets/icons/settings.svg")),
            "native-icons/menu.svg" => Some(include_bytes!("../assets/icons/menu.svg").as_slice()),
            "native-icons/collection.svg" => {
                Some(include_bytes!("../assets/icons/collection.svg").as_slice())
            }
            "native-icons/search.svg" => Some(include_bytes!("../assets/icons/search.svg")),
            "native-icons/expand.svg" => Some(include_bytes!("../assets/icons/expand.svg")),
            "native-icons/shrink.svg" => Some(include_bytes!("../assets/icons/shrink.svg")),
            "native-icons/visualizer.svg" => Some(include_bytes!("../assets/icons/visualizer.svg")),
            "native-icons/microphone.svg" => Some(include_bytes!("../assets/icons/microphone.svg")),
            "native-icons/queue.svg" => Some(include_bytes!("../assets/icons/queue.svg")),
            "native-icons/related.svg" => Some(include_bytes!("../assets/icons/related.svg")),
            "native-icons/sliders.svg" => Some(include_bytes!("../assets/icons/sliders.svg")),
            "native-icons/volume-low.svg" => Some(include_bytes!("../assets/icons/volume-low.svg")),
            "native-icons/volume-high.svg" => {
                Some(include_bytes!("../assets/icons/volume-high.svg"))
            }
            "native-icons/volume-muted.svg" => {
                Some(include_bytes!("../assets/icons/volume-muted.svg"))
            }
            "native-icons/rating-outline.svg" => {
                Some(include_bytes!("../assets/icons/rating-outline.svg"))
            }
            "native-icons/rating-fill.svg" => {
                Some(include_bytes!("../assets/icons/rating-fill.svg"))
            }
            _ => None,
        };
        if let Some(bytes) = bytes {
            Ok(Some(Cow::Borrowed(bytes)))
        } else {
            gpui_component_assets::Assets.load(path)
        }
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = gpui_component_assets::Assets.list(path)?;
        if path.is_empty() || path.trim_end_matches('/') == "native-icons" {
            assets.extend(
                PLAYER
                    .into_iter()
                    .chain(INTERFACE)
                    .chain(RATING)
                    .map(|name| format!("native-icons/{name}.svg").into()),
            );
        }
        Ok(assets)
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetSource, Assets, INTERFACE, PLAYER, RATING};
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_app_icon_loads_at_full_resolution() {
        objc2::rc::autoreleasepool(|_| {
            let icon = super::macos_app_icon().unwrap();
            assert!(icon.isValid());
            assert!(
                icon.representations()
                    .iter()
                    .any(|rep| rep.pixelsWide() == 1024 && rep.pixelsHigh() == 1024)
            );
        });
    }

    #[test]
    fn original_player_icons_render_at_native_and_double_sizes() {
        for name in PLAYER.into_iter().chain(INTERFACE).chain(RATING) {
            let source = Assets
                .load(&format!("native-icons/{name}.svg"))
                .unwrap()
                .unwrap();
            assert!(
                !std::str::from_utf8(&source)
                    .unwrap()
                    .contains("strokeWidth")
            );
            for scale in [1., 2.] {
                let tree = resvg::usvg::Tree::from_data(&source, &resvg::usvg::Options::default())
                    .unwrap();
                let size = (24. * scale) as u32;
                let mut image = resvg::tiny_skia::Pixmap::new(size, size).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::from_scale(scale, scale),
                    &mut image.as_mut(),
                );
                assert_eq!(image.width(), (24. * scale) as u32);
                assert!(
                    image
                        .pixels()
                        .iter()
                        .filter(|pixel| pixel.alpha() > 0)
                        .count()
                        >= if name == "window-minimize" { 28 } else { 31 },
                    "{name}"
                );
            }
        }
        assert!(Assets.load("icons/check.svg").unwrap().is_some());
    }
}
