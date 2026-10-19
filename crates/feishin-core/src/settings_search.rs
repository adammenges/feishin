pub const SETTINGS_TITLES: &[(&str, &[&str])] = &[
    ("analytics-settings", &["setting.analyticsEnable"]),
    (
        "application-settings",
        &[
            "setting.language",
            "setting.fontType",
            "setting.font",
            "setting.font",
            "setting.customFontPath",
            "setting.zoom",
            "setting.savePlayQueue",
            "setting.confirmQueueChanges",
            "setting.homeFeature",
            "setting.homeFeatureStyle",
            "setting.albumBackground",
            "setting.albumBackgroundBlur",
            "setting.artistBackground",
            "setting.artistBackgroundBlur",
            "setting.imageAspectRatio",
            "setting.sidePlayQueueStyle",
            "setting.sidePlayQueueLayout",
            "setting.showFavorites",
            "setting.showRatings",
            "setting.blurExplicitImages",
            "setting.imagePlaceholderPriority",
            "setting.enableGridMultiSelect",
            "setting.playerbarOpenDrawer",
            "setting.fullscreenAutoOpenTimeout",
            "setting.autosave",
            "setting.autosaveCount",
        ],
    ),
    (
        "audio-settings",
        &[
            "setting.audioPlayer",
            "setting.audioDevice",
            "setting.webAudio",
            "setting.preservePitch",
            "setting.audioFadeOnStatusChange",
        ],
    ),
    (
        "auto-dj-settings",
        &[
            "setting.autoDJ_itemCount",
            "setting.autoDJ_mode",
            "setting.autoDJ_songStrategy",
            "setting.autoDJ_albumStrategy",
            "setting.autoDJ_timing",
            "setting.autoDJ_allowDuplicates",
            "setting.autoDJ_onlySimilar",
        ],
    ),
    (
        "cache-settngs",
        &["setting.clearQueryCache", "setting.clearCache"],
    ),
    (
        "control-settings",
        &[
            "setting.buttonSize",
            "setting.showSkipButtons",
            "setting.skipDuration",
            "setting.playButtonBehavior",
            "setting.upNextQueue",
            "setting.followCurrentSong",
            "setting.sidebarImageEnabled",
            "setting.artistRadioCount",
            "setting.volumeWheelStep",
            "setting.volumeWidth",
            "setting.playerbarSlider",
            "setting.playerbarWaveformAlign",
            "setting.playerbarWaveformBarWidth",
            "setting.playerbarWaveformGap",
            "setting.playerbarWaveformRadius",
            "setting.playerbarWaveformStretch",
            "setting.waveformLoadingDelay",
            "setting.microtonalPitchControls",
        ],
    ),
    (
        "discord-settings",
        &[
            "setting.discordRichPresence",
            "setting.discordApplicationId",
            "setting.discordPausedStatus",
            "setting.discordStateIcon",
            "setting.discordListening",
            "setting.discordDisplayType",
            "setting.discordLinkType",
            "setting.discordServeImage",
            "setting.lastfmApiKey",
        ],
    ),
    (
        "eq-settings",
        &[
            "setting.equalizer",
            "setting.equalizerPreset",
            "setting.equalizerSavePreset",
            "setting.equalizerPreamp",
            "setting.equalizerBands",
            "setting.compressorThreshold",
            "setting.compressorRatio",
            "setting.compressorAttack",
            "setting.compressorRelease",
            "setting.compressorMakeupGain",
            "setting.compressorKnee",
            "setting.compressor",
            "setting.equalizerPreset",
            "setting.equalizerSavePreset",
        ],
    ),
    (
        "export-import-settings",
        &[
            "setting.exportImportSettings_importModalTitle",
            "setting.exportImportSettings_control_title",
        ],
    ),
    (
        "external-links-settings",
        &[
            "setting.externalLinks",
            "setting.lastfm",
            "setting.listenbrainz",
            "setting.musicbrainz",
            "setting.qobuz",
            "setting.spotify",
            "setting.nativeSpotify",
        ],
    ),
    ("logger-settings", &["setting.logLevel"]),
    (
        "lyric-settings",
        &[
            "setting.followLyric",
            "setting.preferLocalLyrics",
            "setting.lyricFetch",
            "setting.lyricFetchProvider",
            "setting.enableFurigana",
            "setting.enableRomaji",
            "setting.neteaseTranslation",
            "setting.lyricOffset",
            "setting.translationTargetLanguage",
            "setting.translationApiProvider",
            "setting.translationApiKey",
            "setting.enableAutoTranslation",
        ],
    ),
    ("media-session-settings", &["setting.mediaSession"]),
    (
        "mpv-settings",
        &[
            "setting.mpvExecutablePath",
            "setting.mpvExtraParameters",
            "setting.gaplessAudio",
            "setting.sampleRate",
            "setting.audioExclusiveMode",
            "setting.replayGainMode",
            "setting.replayGainPreamp",
            "setting.replayGainClipping",
            "setting.replayGainFallback",
        ],
    ),
    ("password-settings", &["setting.passwordStore"]),
    ("player-filter-settings", &["setting.playerFilters"]),
    (
        "query-builder-settings",
        &["setting.queryBuilderCustomFields"],
    ),
    (
        "remote-settings",
        &[
            "setting.enableRemote",
            "setting.remotePort",
            "setting.remoteUsername",
            "setting.remotePassword",
        ],
    ),
    (
        "scrobble-settings",
        &[
            "setting.scrobble",
            "setting.scrobbleMinimumMode",
            "setting.minimumScrobblePercentage",
            "setting.minimumScrobbleSeconds",
            "setting.notify",
        ],
    ),
    (
        "sidebar-settings",
        &[
            "setting.sidebarPlaylistList",
            "setting.sidebarPlaylistListFilterRegex",
            "setting.sidebarPlaylistSorting",
            "setting.sidebarPlaylistMode",
            "setting.sidebarPlaylistFolders",
            "setting.sidebarPlaylistFolderSeparator",
            "setting.sidebarPlaylistFolderView",
            "setting.sidebarPlaylistFolderTreeIndent",
            "setting.sidebarPlaylistFolderTreeLineColor",
            "setting.sidebarCollapsedNavigation",
            "setting.showQueueInSidebar",
            "setting.showLyricsInSidebar",
            "setting.showVisualizerInSidebar",
            "setting.combinedLyricsAndVisualizer",
        ],
    ),
    (
        "theme-settings",
        &[
            "setting.solarizedAppearance",
            "setting.useSystemTheme",
            "setting.theme",
            "setting.themeDark",
            "setting.themeLight",
            "setting.useThemeAccentColor",
            "setting.accentColor",
            "setting.useThemePrimaryShade",
            "setting.primaryShade",
        ],
    ),
    (
        "transcode-settings",
        &[
            "setting.transcode",
            "setting.transcodeBitrate",
            "setting.transcodeFormat",
            "setting.transcodeSampleRate",
        ],
    ),
    (
        "update-settings",
        &["setting.releaseChannel", "setting.automaticUpdates"],
    ),
    ("window-hotkey-settings", &["setting.globalMediaHotkeys"]),
    (
        "window-settings",
        &[
            "setting.windowBarStyle",
            "setting.windowBarTrackinfo",
            "setting.trayEnabled",
            "setting.minimizeToTray",
            "setting.exitToTray",
            "setting.startMinimized",
            "setting.preventSleepOnPlayback",
            "setting.preventSuspendOnPlayback",
        ],
    ),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    Playback,
    Hotkeys,
    Window,
    Advanced,
    #[default]
    #[serde(other)]
    General,
}

impl Tab {
    pub const ALL: [Self; 5] = [
        Self::General,
        Self::Playback,
        Self::Hotkeys,
        Self::Window,
        Self::Advanced,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Playback => "playback",
            Self::Hotkeys => "hotkeys",
            Self::Window => "window",
            Self::Advanced => "advanced",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::General => "page.setting.generalTab",
            Self::Playback => "page.setting.playbackTab",
            Self::Hotkeys => "page.setting.hotkeysTab",
            Self::Window => "page.setting.windowTab",
            Self::Advanced => "page.setting.advanced",
        }
    }
}

pub struct Section {
    pub tab: Tab,
    pub group: &'static str,
    pub title: &'static str,
}

// The order and membership follow the original five *-tab.tsx components.
pub const SECTIONS: &[Section] = &[
    Section {
        tab: Tab::General,
        group: "theme-settings",
        title: "page.setting.theme",
    },
    Section {
        tab: Tab::General,
        group: "application-settings",
        title: "page.setting.application",
    },
    Section {
        tab: Tab::General,
        group: "external-links-settings",
        title: "common.externalLinks",
    },
    Section {
        tab: Tab::General,
        group: "control-settings",
        title: "page.setting.controls",
    },
    Section {
        tab: Tab::General,
        group: "sidebar-settings",
        title: "page.setting.sidebar",
    },
    Section {
        tab: Tab::General,
        group: "scrobble-settings",
        title: "page.setting.scrobble",
    },
    Section {
        tab: Tab::General,
        group: "lyric-settings",
        title: "page.setting.lyrics",
    },
    Section {
        tab: Tab::General,
        group: "query-builder-settings",
        title: "page.setting.queryBuilder",
    },
    Section {
        tab: Tab::Playback,
        group: "audio-settings",
        title: "page.setting.audio",
    },
    Section {
        tab: Tab::Playback,
        group: "mpv-settings",
        title: "",
    },
    Section {
        tab: Tab::Playback,
        group: "eq-settings",
        title: "",
    },
    Section {
        tab: Tab::Playback,
        group: "transcode-settings",
        title: "page.setting.transcoding",
    },
    Section {
        tab: Tab::Playback,
        group: "player-filter-settings",
        title: "page.setting.playerFilters",
    },
    Section {
        tab: Tab::Playback,
        group: "auto-dj-settings",
        title: "setting.autoDJ",
    },
    Section {
        tab: Tab::Hotkeys,
        group: "window-hotkey-settings",
        title: "",
    },
    Section {
        tab: Tab::Hotkeys,
        group: "media-session-settings",
        title: "",
    },
    Section {
        tab: Tab::Hotkeys,
        group: "hotkey-manager-settings",
        title: "setting.applicationHotkeys",
    },
    Section {
        tab: Tab::Window,
        group: "window-settings",
        title: "page.setting.application",
    },
    Section {
        tab: Tab::Window,
        group: "discord-settings",
        title: "page.setting.discord",
    },
    Section {
        tab: Tab::Window,
        group: "remote-settings",
        title: "page.setting.remote",
    },
    Section {
        tab: Tab::Window,
        group: "password-settings",
        title: "",
    },
    Section {
        tab: Tab::Advanced,
        group: "update-settings",
        title: "page.setting.updates",
    },
    Section {
        tab: Tab::Advanced,
        group: "analytics-settings",
        title: "page.setting.analytics",
    },
    Section {
        tab: Tab::Advanced,
        group: "export-import-settings",
        title: "page.setting.exportImport",
    },
    Section {
        tab: Tab::Advanced,
        group: "logger-settings",
        title: "page.setting.logger",
    },
    Section {
        tab: Tab::Advanced,
        group: "cache-settngs",
        title: "page.setting.cache",
    },
];

pub fn section(titles: &[&str]) -> Option<usize> {
    let index = |group| {
        let group = match group {
            "styles-settings" => "theme-settings",
            "home-settings"
            | "artist-settings"
            | "path-settings"
            | "art-resolution-settings"
            | "fullscreen-player-settings" => "application-settings",
            "Private mode" => "scrobble-settings",
            "Appearance" | "Custom CSS" => "theme-settings",
            "Playback"
            | "FFmpeg"
            | "Playback speed"
            | "pitch-adjust-{i}"
            | "Track transition"
            | "Sleep timer"
            | "Native audio streams common music formats and live radio. MPV also supports ReplayGain and audio filters." => {
                "audio-settings"
            }
            "Microtonal pitch controls" | "Play button behavior" => "control-settings",
            "Confirm fetching an entire library list"
            | "Edit local audio tags"
            | "Settings"
            | "Apply path mapping"
            | "Scan server library"
            | "Collections"
            | "Visualizer"
            | "Album groups" => "application-settings",
            "External links" => "external-links-settings",
            "Equalizer" | "Reset equalizer" => "eq-settings",
            "Remote control" => "remote-settings",
            "Library and data" | "Open application data folder" => "cache-settngs",
            "Export diagnostics" | "Open log folder" | "Restart" | "Developer tools" => {
                "logger-settings"
            }
            "Keyboard shortcuts" => "hotkey-manager-settings",
            group => group,
        };
        SECTIONS.iter().position(|section| section.group == group)
    };
    // An explicit destination also classifies native-only controls and section links.
    titles.iter().find_map(|title| index(*title)).or_else(|| {
        titles.iter().find_map(|title| {
            SETTINGS_TITLES
                .iter()
                .find_map(|(group, keys)| keys.contains(title).then(|| index(group)).flatten())
        })
    })
}

/// The original settings search matches case-insensitive translated option titles.
/// A native section link also matches the titles of the settings it opens.
pub fn matches(language: &str, query: &str, titles: &[&str]) -> bool {
    let query = query.to_lowercase();
    query.is_empty()
        || titles.iter().any(|title| {
            crate::i18n::text(language, title)
                .to_lowercase()
                .contains(&query)
                || SETTINGS_TITLES
                    .iter()
                    .find(|(group, _)| group == title)
                    .is_some_and(|(_, keys)| {
                        keys.iter().any(|key| {
                            crate::i18n::text(language, key)
                                .to_lowercase()
                                .contains(&query)
                        })
                    })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_tabs_preserve_original_categories_and_imported_selection() {
        for (keys, tab) in [
            (vec!["setting.language"], Tab::General),
            (vec!["setting.volumeWidth"], Tab::General),
            (vec!["setting.scrobble"], Tab::General),
            (vec!["setting.mpvExecutablePath"], Tab::Playback),
            (vec!["setting.equalizer"], Tab::Playback),
            (vec!["setting.globalMediaHotkeys"], Tab::Hotkeys),
            (vec!["Keyboard shortcuts"], Tab::Hotkeys),
            (vec!["setting.trayEnabled"], Tab::Window),
            (vec!["setting.remotePort"], Tab::Window),
            (vec!["update-settings"], Tab::Advanced),
            (vec!["cache-settngs"], Tab::Advanced),
            (vec!["setting.logLevel"], Tab::Advanced),
        ] {
            assert_eq!(
                SECTIONS[section(&keys).expect("Original setting is categorized")].tab,
                tab,
                "{keys:?}"
            );
        }
        for tab in Tab::ALL {
            let original = serde_json::json!({ "general": {}, "tab": tab.id() });
            let preview = crate::migration::preview(
                &crate::storage::Settings::default(),
                &serde_json::to_vec(&original).unwrap(),
            )
            .unwrap();
            assert_eq!(preview.settings.settings_tab, tab);
            assert!(!preview.omitted.iter().any(|key| key == "tab"));
            let saved = serde_json::to_vec(&preview.settings).unwrap();
            assert_eq!(
                serde_json::from_slice::<crate::storage::Settings>(&saved)
                    .unwrap()
                    .settings_tab,
                tab
            );
        }
        assert_eq!(
            serde_json::from_str::<Tab>("\"removed\"").unwrap(),
            Tab::General
        );
    }
    #[test]
    fn search_reuses_original_localized_titles_and_section_destinations() {
        assert!(
            SETTINGS_TITLES
                .iter()
                .map(|(_, keys)| keys.len())
                .sum::<usize>()
                > 150
        );
        assert!(matches("en", "EQUALIZER", &["Equalizer"]));
        assert!(!matches("en", " equalizer", &["Equalizer"]));
        assert!(matches("en", "", &["Anything"]));
        assert!(matches("en", "translation", &["lyric-settings"]));
        assert!(!matches("en", "translation", &["theme-settings"]));
        assert!(matches("fr", "traduction", &["lyric-settings"]));
        assert!(matches("fr", "langage", &["setting.language"]));
        assert_eq!(crate::i18n::text("fr", "Language"), "Langue");
        assert_eq!(crate::i18n::text("fr", "setting.language"), "Langage");
        assert!(!matches("fr", "language", &["setting.language"]));
        assert!(matches("en", "compressor", &["eq-settings"]));
    }
}
