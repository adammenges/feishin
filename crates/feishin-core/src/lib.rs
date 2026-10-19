pub mod api;
pub mod appearance;
pub mod artists;
pub mod dlna;
pub mod dsp;
pub mod external_links;
pub mod genres;
pub mod i18n;
pub mod images;
pub mod information;
pub mod instance;
pub mod japanese;
pub mod jukebox;
pub mod lyric_conversion;
pub mod lyric_export;
mod lyric_match;
pub mod lyric_sources;
pub mod lyrics;
pub mod lyrics_markup;
mod mantine;
mod media_queries;
pub mod metadata_format;
pub mod migration;
pub mod model;
pub mod playback;
pub mod playback_filters;
pub mod player_display;
pub mod power;
pub mod presence;
mod presence_ipc;
pub mod presets;
pub mod proxy;
pub mod queue;
pub mod remote;
pub mod shuffle;
pub mod storage;
pub mod styles;
pub mod tags;
mod tags_taglib;
pub mod tempo;
mod theme_colors;
#[cfg(test)]
mod tls_tests;
pub mod transition;
pub mod waveform;

pub mod shortcuts;

pub mod diagnostics;

pub mod smart;

pub mod playlist_albums;
pub mod playlist_folders;
mod playlist_sync;

pub mod auto_dj;

pub mod sleep_timer;

pub mod releases;

pub mod lists;

pub mod filters;

pub mod cache;
pub mod collections;
pub mod credentials;
pub mod css_transition;

pub mod system_audio;
pub mod visualizer;

pub mod fonts;
mod hepburn;
pub mod home;
pub mod panels;
pub mod release_notes;
pub mod sidebar;
pub mod translation;
pub mod updates;

pub mod server_discovery;
pub mod settings_search;
pub mod sharing;

pub mod album_metadata;

pub mod artist_sections;
