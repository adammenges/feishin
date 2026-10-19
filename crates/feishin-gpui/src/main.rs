mod actions;
mod add_to_playlist;
mod album_groups;
mod app;
mod app_menu;
mod appearance;
mod artist_overview;
mod artists;
mod auto_dj;
mod cache;
mod collections;
mod command_palette;
mod compressor;
mod context_preview;
mod credentials;
mod details;
mod editor;
mod equalizer;
#[cfg(test)]
mod equalizer_tests;
mod expanded_album;
mod filters;
mod folders;
mod fonts;
mod genres;
mod grid_carousel;
mod home;
mod home_genres;
mod icons;
mod images;
mod item_card;
mod library;
mod library_grid;
mod library_header;
mod list_header;
mod lists;
mod lyric_export;
mod lyric_scroll;
mod lyrics;
mod lyrics_highlight;
mod lyrics_markup;
mod lyrics_row;
mod lyrics_status;
#[cfg(all(target_os = "macos", not(test)))]
mod macos_window;
mod media;
mod milkdrop;
mod navigation;
mod notifications;
mod outputs;
mod panels;
mod path_settings;
mod playback_controls;
mod playback_filters;
mod player_config;
mod player_display;
mod player_layout;
mod player_related;
mod playerbar;
mod playlist_actions;
mod playlist_header;
mod playlists;
mod presence_settings;
mod progress;
mod queue;
mod quick_connect;
mod radio;
mod rating;
mod release_notes;
mod row_play;
mod scan;
mod scrobble;
mod scroll;
mod servers;
mod settings;
#[cfg(test)]
mod settings_import_tests;
mod sharing;
mod shortcuts;
mod shuffle;
mod sidebar;
mod sidebar_artwork;
mod sidebar_motion;
mod sleep;
#[cfg(test)]
mod slider_tests;
mod smart;
mod speaker_handoff;
mod speaker_picker;
mod speaker_properties;
mod speaker_volume;
mod styles;
mod taskbar;
#[cfg(test)]
mod tests;
mod theme_settings;
mod tray;
mod ui;
mod updates;
mod visualizer;
mod visualizer_display;
mod volume;
mod waveform;
mod window;

use crate::icons::Assets;
use app::{Feishin, LeaveFullPlayer, Quit};
use gpui::*;
use gpui_component::{Root, Theme, ThemeMode};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if first
        .as_ref()
        .is_some_and(|argument| argument == "--apply-native-update")
    {
        let job = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("Missing native update job"))?;
        return feishin_core::updates::run_installer(std::path::Path::new(&job));
    }
    if first
        .as_ref()
        .is_some_and(|argument| argument == "--restart-after")
    {
        let parent = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("Missing restart parent process"))?;
        let parent = parent
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid restart parent process"))?
            .parse()?;
        feishin_core::instance::wait_for_exit(parent, Duration::from_secs(120))?;
    }
    let data_dir = feishin_core::storage::data_dir()?;
    let Some(instance) = feishin_core::instance::Instance::start(&data_dir)? else {
        return Ok(());
    };
    let state = feishin_core::storage::load(&data_dir.join("state.json"));
    feishin_core::diagnostics::configure(&data_dir)?;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace"))
        .format(|buffer, record| {
            use std::io::Write;
            // Tracing bridges can submit records without checking log's live limit.
            if record.level() > log::max_level() {
                return Ok(());
            }
            let line = feishin_core::diagnostics::record(
                record.level(),
                record.target(),
                &record.args().to_string(),
            );
            write!(buffer, "{line}")
        })
        .init();
    log::set_max_level(
        if state
            .as_ref()
            .is_ok_and(|state| state.settings.log_level == "debug")
        {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        },
    );
    log::info!("Feishin {} starting with GPUI", env!("CARGO_PKG_VERSION"));
    #[cfg(windows)]
    if let Err(error) = taskbar::configure_app_id() {
        log::warn!("Cannot configure Windows application identity: {error}");
    }
    feishin_core::credentials::configure(
        state
            .as_ref()
            .map_or(Default::default(), |state| state.settings.password_store),
    );
    let smoke = std::env::args().any(|arg| arg == "--smoke-test");
    let application = Application::new().with_assets(Assets);
    application.on_reopen(app_menu::reopen);
    application.run(move |cx| {
        #[cfg(target_os = "macos")]
        if let Err(error) = icons::set_macos_app_icon() {
            log::warn!("Cannot set the macOS app icon: {error}");
        }
        gpui_component::init(cx);
        if let Err(error) = fonts::register_builtins(cx) {
            log::warn!("Cannot load built-in fonts: {error}");
        }
        let dark = state.as_ref().map_or(true, |s| s.settings.theme != "light");
        Theme::change(
            if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            None,
            cx,
        );
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new(
                "escape",
                LeaveFullPlayer,
                Some("Feishin && !RecordingShortcut"),
            ),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = window::initial_bounds(state.as_ref().ok().and_then(|state| state.window), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(bounds),
                titlebar: Some(window::titlebar_options(
                    state
                        .as_ref()
                        .map_or("linux", |state| state.settings.window_bar_style.as_str()),
                )),
                window_decorations: Some(
                    if state
                        .as_ref()
                        .is_ok_and(|state| state.settings.window_bar_style != "linux")
                    {
                        WindowDecorations::Client
                    } else {
                        WindowDecorations::Server
                    },
                ),
                window_min_size: Some(size(px(960.0), px(640.0))),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| Feishin::new(data_dir, state, window, cx));
                view.update(cx, |view, _| view.instance = Some(instance));
                cx.new(|cx| Root::new(view, window, cx))
            },
        );
        if let Err(error) = result {
            log::error!("Cannot open Feishin: {error}");
            cx.quit();
        }
        cx.activate(true);
        if smoke {
            cx.spawn(async move |cx| {
                smol::Timer::after(Duration::from_secs(3)).await;
                cx.update(|cx| cx.quit()).ok();
            })
            .detach();
        }
    });
    Ok(())
}

#[cfg(all(test, unix))]
mod test_speaker;
