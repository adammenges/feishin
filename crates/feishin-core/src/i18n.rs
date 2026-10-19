use serde_json::Value;
use std::{collections::HashMap, sync::OnceLock};

include!(concat!(env!("OUT_DIR"), "/locales.rs"));

pub(crate) fn system_locale() -> &'static str {
    static LOCALE: OnceLock<String> = OnceLock::new();
    LOCALE.get_or_init(|| sys_locale::get_locale().unwrap_or_else(|| "en-US".into()))
}

fn collator(language: &str, numeric: bool) -> icu_collator::CollatorBorrowed<'static> {
    let locale = language
        .parse::<icu_locale_core::Locale>()
        .unwrap_or_else(|_| "en-US".parse().unwrap());
    let mut preferences = icu_collator::CollatorPreferences::from(locale);
    preferences.numeric_ordering = Some(if numeric {
        icu_collator::preferences::CollationNumericOrdering::True
    } else {
        icu_collator::preferences::CollationNumericOrdering::False
    });
    icu_collator::Collator::try_new(preferences, Default::default()).unwrap()
}

pub fn compare(a: &str, b: &str, numeric: bool) -> std::cmp::Ordering {
    static COLLATOR: OnceLock<[icu_collator::CollatorBorrowed<'static>; 2]> = OnceLock::new();
    COLLATOR.get_or_init(|| {
        [
            collator(system_locale(), false),
            collator(system_locale(), true),
        ]
    })[usize::from(numeric)]
    .compare(a, b)
}

fn locale_lowercase<'a>(
    value: &'a str,
    locale: &icu_locale_core::Locale,
) -> std::borrow::Cow<'a, str> {
    icu_casemap::CaseMapper::new().lowercase_to_string(value, &locale.id)
}

pub fn compare_lowercase(a: &str, b: &str, numeric: bool) -> std::cmp::Ordering {
    static LOCALE: OnceLock<icu_locale_core::Locale> = OnceLock::new();
    let locale = LOCALE.get_or_init(|| {
        system_locale()
            .parse()
            .unwrap_or_else(|_| "en-US".parse().unwrap())
    });
    compare(
        &locale_lowercase(a, locale),
        &locale_lowercase(b, locale),
        numeric,
    )
}

pub(crate) fn title_case(value: &str) -> String {
    static WORDS: OnceLock<regex::Regex> = OnceLock::new();
    // JavaScript /\w\S*/ uses ASCII word starts and ECMAScript whitespace.
    let words = WORDS.get_or_init(|| {
        regex::Regex::new(r"[A-Za-z0-9_][^\x09-\x0d\x20\u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]*").unwrap()
    });
    words
        .replace_all(value, |word: &regex::Captures<'_>| {
            format!(
                "{}{}",
                word[0][..1].to_uppercase(),
                word[0][1..].to_lowercase()
            )
        })
        .into_owned()
}

fn catalogs() -> &'static HashMap<&'static str, Value> {
    static CATALOGS: OnceLock<HashMap<&str, Value>> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        LOCALES
            .iter()
            .map(|(id, json)| {
                (
                    *id,
                    serde_json::from_str(json).expect("Invalid bundled locale"),
                )
            })
            .collect()
    })
}

pub fn languages() -> Vec<&'static str> {
    LOCALES.iter().map(|(id, _)| *id).collect()
}

pub fn translate(language: &str, key: &str) -> Option<String> {
    translate_with(language, key, &serde_json::json!({}))
}

pub fn translate_with(language: &str, key: &str, options: &Value) -> Option<String> {
    resolve(language, key, options, 0)
}

fn resolve(language: &str, key: &str, options: &Value, depth: usize) -> Option<String> {
    if depth >= 16 || key.len() > 4096 || language.len() > 256 {
        return None;
    }
    let language = language.replace('_', "-");
    let mut locales = vec![language.as_str()];
    let mut parent = language.as_str();
    while let Some((base, _)) = parent.rsplit_once('-') {
        locales.push(base);
        parent = base;
    }
    locales.push("en");
    for locale in locales {
        let Some(catalog) = catalogs().get(locale) else {
            continue;
        };
        let context = match &options["context"] {
            Value::String(context) if !context.is_empty() && context.len() <= 4096 => {
                Some(context.clone())
            }
            Value::Number(context) if context.as_f64() != Some(0.) => Some(context.to_string()),
            _ => None,
        };
        let mut keys = Vec::new();
        for base in context
            .map(|context| format!("{key}_{context}"))
            .into_iter()
            .chain(std::iter::once(key.to_owned()))
        {
            if let Some(count) = options["count"].as_number() {
                if count.as_f64() == Some(0.) {
                    keys.push(format!("{base}_zero"));
                }
                if let Some(category) = count
                    .as_f64()
                    .and_then(|count| plural(locale, &count.to_string()))
                {
                    keys.push(format!("{base}_{category}"));
                }
            }
            keys.push(base);
        }
        for key in keys {
            let pointer = format!("/{}", key.replace('.', "/"));
            if let Some(text) = catalog.pointer(&pointer).and_then(Value::as_str) {
                return render(&language, text, options, depth);
            }
        }
    }
    None
}

pub(crate) fn plural(language: &str, count: &str) -> Option<&'static str> {
    use icu_plurals::{PluralCategory, PluralOperands, PluralRules};
    let locale = icu_locale_core::Locale::try_from_str(language).ok()?;
    let rules = PluralRules::try_new_cardinal(locale.into()).ok()?;
    let number = count.parse::<f64>().ok()?;
    let rounded = (number * 1000.).round() / 1000.;
    let decimal = rounded.to_string().parse::<fixed_decimal::Decimal>().ok()?;
    let operands = PluralOperands::from(&decimal);
    Some(match rules.category_for(operands) {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        _ => "other",
    })
}

fn interpolate(text: &str, options: &Value) -> String {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let pattern =
        PATTERN.get_or_init(|| regex::Regex::new(r"\{\{\s*-?\s*([^{}]+?)\s*\}\}").unwrap());
    pattern
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let key = captures[1].trim();
            let value = options.pointer(&format!("/{}", key.replace('.', "/")));
            match value {
                Some(Value::String(value)) => value.clone(),
                Some(value) if value.is_number() || value.is_boolean() => value.to_string(),
                _ => captures[0].to_owned(),
            }
        })
        .into_owned()
}

fn render(language: &str, text: &str, options: &Value, depth: usize) -> Option<String> {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| regex::Regex::new(r"\$t\(([^()]*)\)").unwrap());
    let text = interpolate(text, options);
    let mut result = String::new();
    let mut end = 0;
    for (index, captures) in pattern.captures_iter(&text).enumerate() {
        if index >= 128 {
            return None;
        }
        let matched = captures.get(0)?;
        result.push_str(&text[end..matched.start()]);
        let (key, overrides) = captures[1]
            .split_once(',')
            .map_or((captures[1].trim(), None), |(key, json)| {
                (key.trim(), serde_json::from_str::<Value>(json).ok())
            });
        let mut nested_options = options.clone();
        if let (Some(target), Some(source)) = (
            nested_options.as_object_mut(),
            overrides.and_then(|value| value.as_object().cloned()),
        ) {
            target.extend(source);
        }
        result.push_str(
            &resolve(language, key, &nested_options, depth + 1).unwrap_or_else(|| key.to_owned()),
        );
        if result.len() > 1_000_000 {
            return None;
        }
        end = matched.end();
    }
    result.push_str(&text[end..]);
    Some(result)
}

pub fn text(language: &str, english: &str) -> String {
    if let Some(text) = caption(language, english) {
        return text;
    }
    if language.split(['-', '_']).next() != Some("en") {
        let alias = match english {
            "Play now" => Some("player.play"),
            "Play next" => Some("player.addNext"),
            "Add last" | "Add to queue" => Some("player.addLast"),
            "Shuffle play" => Some("player.shuffle"),
            "Shuffle next" => Some("player.addNextShuffled"),
            "Shuffle last" => Some("player.addLastShuffled"),
            "Information" => Some("page.contextMenu.showDetails"),
            "Edit audio tags" => Some("page.contextMenu.editMetadata"),
            "Download original file" => Some("page.contextMenu.download"),
            "Move next" => Some("action.moveToNext"),
            "Move to end" => Some("action.moveToBottom"),
            "Add field" => Some("page.itemDetail.addField"),
            "Add group" => Some("form.queryEditor.addRuleGroup"),
            "Remove group" => Some("form.queryEditor.removeRuleGroup"),
            "Visual rule editor" => Some("form.queryEditor.title"),
            "Add station" => Some("action.createRadioStation"),
            "Album artwork size" => Some("table.config.general.albumImageSize"),
            "Album group metadata" => Some("table.config.general.albumGroupMetadata"),
            "Album groups" => Some("table.config.label.albumGroup"),
            "Autosave queue to server" => Some("setting.autosave"),
            "Cancel sleep timer" => Some("player.sleepTimer_cancel"),
            "Cards" => Some("table.config.view.grid"),
            "Customize home page" => Some("setting.homeConfiguration"),
            "Home page settings" => Some("setting.homeConfiguration"),
            "Hide playlist names matching regex"
            | "Hide playlist names matching this regular expression" => {
                Some("setting.sidebarPlaylistListFilterRegex")
            }
            "New server" => Some("form.addServer.title"),
            "Release notes" => Some("common.viewReleaseNotes"),
            "Remote control" => Some("page.setting.remote"),
            "Remote password" => Some("setting.remotePassword"),
            "No lyrics available" => Some("page.fullscreenPlayer.noLyrics"),
            "See all" => Some("page.albumArtistDetail.viewAll"),
            "New playlist" => Some("action.createPlaylist"),
            "Edit playlist" => Some("action.editPlaylist"),
            "No items" => Some("common.noResultsFromQuery"),

            "Change rating" => Some("page.contextMenu.setRating"),
            "Clear filter" => Some("form.queryEditor.clearFilters"),
            "Clear selection" => Some("action.deselectAll"),
            "Close to tray" => Some("setting.exitToTray"),
            "Click player bar to expand" => Some("setting.playerbarOpenDrawer"),
            "Fade on play, pause and stop" => Some("setting.audioFadeOnStatusChange"),
            "Keep display awake during playback" => Some("setting.preventSleepOnPlayback"),
            "MPV arguments and properties (JSON)" => Some("setting.mpvExtraParameters"),
            "Open expanded player after idle minutes (0 disables)" => {
                Some("setting.fullscreenAutoOpenTimeout")
            }
            "Player Filters" => Some("setting.playerFilters"),
            "Playlist folder view" => Some("setting.sidebarPlaylistFolderView"),
            "Playlist folders" => Some("setting.sidebarPlaylistFolders"),
            "Prevent idle sleep during playback" => Some("setting.preventSuspendOnPlayback"),
            "Restore queue on startup" => Some("setting.savePlayQueue"),
            "Scrobble threshold mode" => Some("setting.scrobbleMinimumMode"),
            "Scrobble to server" => Some("setting.scrobble"),
            "Tracks between queue saves" => Some("setting.autosaveCount"),
            "Transcode format" => Some("setting.transcodeFormat"),
            "Window bar style" => Some("setting.windowBarStyle"),
            "Configure player" => Some("common.configure"),
            "Custom query builder fields" => Some("setting.queryBuilderCustomFields"),
            "Discord client ID" | "Client ID" => Some("setting.discordApplicationId"),
            "Discord Rich Presence settings" => Some("setting.discordRichPresence"),
            "Download update" => Some("page.contextMenu.download"),
            "Edit tags" | "Edit local audio tags" => Some("page.contextMenu.editMetadata"),
            "Export settings and queue" => Some("setting.exportImportSettings_control_exportText"),
            "Fonts" => Some("setting.font"),
            "Follow system theme" => Some("setting.useSystemTheme"),
            "Keyboard shortcuts" => Some("setting.applicationHotkeys"),
            "Local audio device" => Some("setting.audioDevice"),
            "Loading" | "Loading..." => Some("dlna.speakerProperties.loading"),
            "Next page" => Some("player.next"),
            "Previous page" => Some("player.previous"),
            "Play track" | "Play all" => Some("player.play"),
            "Play/Pause" => Some("setting.hotkey_playbackPlayPause"),
            "Refresh devices" => Some("common.refresh"),
            "Remove all artwork" => Some("page.itemDetail.removeArtwork"),
            "Remove from playlist" => Some("page.contextMenu.removeFromPlaylist"),
            "Remove track" => Some("page.contextMenu.removeFromQueue"),
            "Reset to defaults" | "Restore defaults" | "Restore group defaults" => {
                Some("common.resetToDefault")
            }
            "Restore from server" => Some("player.restoreQueueFromServer"),
            "Save as playlist" => Some("action.createPlaylistFromQueue"),
            "Save collection" => Some("page.collections.saveAsCollection"),
            "Save equalizer preset" | "Save compressor preset" => {
                Some("setting.equalizerSavePreset")
            }
            "Save current spectrum as preset" => Some("visualizer.saveAsPreset"),
            "Save playlist"
            | "Save station"
            | "Save layout"
            | "Save override"
            | "Save field settings"
            | "Save remote settings"
            | "Save server settings"
            | "Save translation settings" => Some("common.save"),
            "Save to server" => Some("player.saveQueueToServer"),
            "Search lyrics" => Some("form.lyricSearch.title"),
            "Servers" => Some("page.manageServers.title"),
            "Set custom timer" => Some("player.sleepTimer_setCustom"),
            "Sidebar settings" => Some("setting.sidebarConfiguration"),
            "Sort order" => Some("common.sortOrder"),
            "Stop playback" => Some("player.stop"),
            "Through year" => Some("form.shuffleAll.input_maxYear"),
            "Tray icon" => Some("setting.trayEnabled"),
            "Undo changes" => Some("common.undo"),
            "Updates" => Some("page.setting.updates"),
            "Appearance" => Some("page.setting.theme"),
            "Background blur" => Some("page.fullscreenPlayer.config.dynamicImageBlur"),
            "Change artwork" => Some("common.edit"),
            "Connect to a server" => Some("form.addServer.title"),
            "Create share link" => Some("common.create"),
            "Edit lyrics" => Some("common.edit"),
            "Enabled" => Some("common.enable"),
            "Export" => Some("form.lyricsExport.export"),
            "Images" => Some("page.itemDetail.artworkTab"),
            "Link type" => Some("setting.discordLinkType"),
            "Loading album tracks..." | "Loading artist information..." => {
                Some("dlna.speakerProperties.loading")
            }
            "Maximum sample rate (Hz, 0 = original)" => Some("setting.transcodeSampleRate"),
            "Open application data folder" => Some("action.openApplicationDirectory"),
            "Queue" => Some("player.viewQueue"),
            "Queue is empty" => Some("common.noResultsFromQuery"),
            "Remove override" => Some("common.reset"),
            "Reset equalizer" => Some("common.resetToDefault"),
            "Show albums" => Some("page.genreList.showAlbums"),
            "Show songs" => Some("page.genreList.showTracks"),
            "Use lyrics" => Some("common.confirm"),
            _ => None,
        };
        if let Some(text) = alias.and_then(|key| caption(language, key)) {
            return text;
        }
    }
    static KEYS: OnceLock<HashMap<String, String>> = OnceLock::new();
    let keys = KEYS.get_or_init(|| {
        fn flatten(value: &Value, path: String, result: &mut HashMap<String, String>) {
            if let Some(object) = value.as_object() {
                for (key, value) in object {
                    flatten(
                        value,
                        if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{path}.{key}")
                        },
                        result,
                    );
                }
            } else if value.is_string()
                && let Some(text) = caption("en", &path)
            {
                result.entry(text.clone()).or_insert_with(|| path.clone());
                result.entry(text.to_lowercase()).or_insert(path);
            }
        }
        let mut result = HashMap::new();
        flatten(&catalogs()["en"], String::new(), &mut result);
        result
    });
    if let Some(translated) = keys
        .get(english)
        .or_else(|| keys.get(&english.to_lowercase()))
        .and_then(|key| caption(language, key))
    {
        return translated;
    }
    if let Some((label, value)) = english.split_once(": ") {
        let translated = text(language, label);
        if translated != label {
            return format!("{translated}: {value}");
        }
    }
    if let Some((label, action)) = english.split_once(" - ") {
        let label = text(language, label);
        let action = text(language, action);
        return format!("{label} - {action}");
    }
    english.to_owned()
}

fn caption(language: &str, key: &str) -> Option<String> {
    static VARIABLES: OnceLock<Value> = OnceLock::new();
    translate_with(language, key, VARIABLES.get_or_init(|| serde_json::json!({
        "discord": "Discord", "lastfm": "last.fm", "musicbrainz": "musicbrainz", "ReplayGain": "ReplayGain"
    })))
}

pub fn rtl(language: &str) -> bool {
    matches!(
        language.split(['-', '_']).next().unwrap_or_default(),
        "ar" | "fa" | "he" | "ur"
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn collation_matches_original_tag_ordering() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/tag-suggestions-reference.json")).unwrap();
        for case in reference["ordering"].as_array().unwrap() {
            let collator = super::collator(
                case["language"].as_str().unwrap(),
                case["numeric"].as_bool().unwrap(),
            );
            let mut values: Vec<String> = serde_json::from_value(case["input"].clone()).unwrap();
            values.sort_by(|a, b| collator.compare(a, b));
            assert_eq!(
                serde_json::to_value(values).unwrap(),
                case["expected"],
                "{case}"
            );
        }
        for case in reference["casing"].as_array().unwrap() {
            let locale: icu_locale_core::Locale =
                case["language"].as_str().unwrap().parse().unwrap();
            let collator = super::collator(
                case["language"].as_str().unwrap(),
                case["numeric"].as_bool().unwrap(),
            );
            let mut values: Vec<String> = serde_json::from_value(case["input"].clone()).unwrap();
            let lower = values
                .iter()
                .map(|value| locale_lowercase(value, &locale).into_owned())
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::to_value(lower).unwrap(),
                case["lowercase"],
                "{case}"
            );
            values.sort_by(|a, b| {
                collator.compare(&locale_lowercase(a, &locale), &locale_lowercase(b, &locale))
            });
            assert_eq!(
                serde_json::to_value(values).unwrap(),
                case["expected"],
                "{case}"
            );
        }
    }
    use super::*;
    #[test]
    fn nested_references_interpolation_and_plurals_match_original_i18next() {
        use std::io::Read;
        let mut decoder = flate2::read::GzDecoder::new(
            include_bytes!("../assets/i18n-reference.json.gz").as_slice(),
        );
        let mut json = String::new();
        decoder.read_to_string(&mut json).unwrap();
        let cases: Vec<Value> = serde_json::from_str(&json).unwrap();
        assert!(cases.len() > 70_000);
        for case in cases {
            let language = case["language"].as_str().unwrap();
            let key = case["key"].as_str().unwrap();
            let actual =
                translate_with(language, key, &case["options"]).unwrap_or_else(|| key.to_owned());
            assert_eq!(
                actual,
                case["expected"].as_str().unwrap(),
                "{language} {key} {:?}",
                case["options"]
            );
        }
        assert_ne!(text("fr", "Add to playlist"), "Add to playlist");
        assert!(!text("fr", "Add to playlist").contains("$t("));
        assert_eq!(
            render("en", "$t(common.refresh)", &serde_json::json!({}), 16),
            Some("common.refresh".into())
        );
        assert!(translate("en", &"x".repeat(4097)).is_none());
    }
    #[test]
    fn native_captions_resolve_original_labels_brand_variables_and_live_values() {
        for language in languages() {
            for key in [
                "player.play",
                "player.addNext",
                "player.addLast",
                "player.holdToShuffle",
            ] {
                assert_eq!(text(language, key), translate(language, key).unwrap());
            }
            for (caption, key, options) in [
                (
                    "Discord Rich Presence",
                    "setting.discordRichPresence",
                    serde_json::json!({"discord":"Discord"}),
                ),
                (
                    "Last.fm API key",
                    "setting.lastfmApiKey",
                    serde_json::json!({"lastfm":"last.fm"}),
                ),
                (
                    "ReplayGain mode",
                    "setting.replayGainMode",
                    serde_json::json!({"ReplayGain":"ReplayGain"}),
                ),
                (
                    "Follow system theme",
                    "setting.useSystemTheme",
                    serde_json::json!({}),
                ),
                ("Close to tray", "setting.exitToTray", serde_json::json!({})),
                (
                    "Customize home page",
                    "setting.homeConfiguration",
                    serde_json::json!({}),
                ),
                (
                    "Home page settings",
                    "setting.homeConfiguration",
                    serde_json::json!({}),
                ),
                (
                    "Hide playlist names matching regex",
                    "setting.sidebarPlaylistListFilterRegex",
                    serde_json::json!({}),
                ),
                (
                    "Hide playlist names matching this regular expression",
                    "setting.sidebarPlaylistListFilterRegex",
                    serde_json::json!({}),
                ),
                (
                    "Remote password",
                    "setting.remotePassword",
                    serde_json::json!({}),
                ),
                ("New server", "form.addServer.title", serde_json::json!({})),
                (
                    "Release notes",
                    "common.viewReleaseNotes",
                    serde_json::json!({}),
                ),
                (
                    "Remote control",
                    "page.setting.remote",
                    serde_json::json!({}),
                ),
                (
                    "No lyrics available",
                    "page.fullscreenPlayer.noLyrics",
                    serde_json::json!({}),
                ),
                (
                    "See all",
                    "page.albumArtistDetail.viewAll",
                    serde_json::json!({}),
                ),
                (
                    "New playlist",
                    "action.createPlaylist",
                    serde_json::json!({}),
                ),
                (
                    "Edit playlist",
                    "action.editPlaylist",
                    serde_json::json!({}),
                ),
                (
                    "No items",
                    "common.noResultsFromQuery",
                    serde_json::json!({}),
                ),
                (
                    "Keep display awake during playback",
                    "setting.preventSleepOnPlayback",
                    serde_json::json!({}),
                ),
                (
                    "Restore queue on startup",
                    "setting.savePlayQueue",
                    serde_json::json!({}),
                ),
                (
                    "Tracks between queue saves",
                    "setting.autosaveCount",
                    serde_json::json!({}),
                ),
                (
                    "Click player bar to expand",
                    "setting.playerbarOpenDrawer",
                    serde_json::json!({}),
                ),
                (
                    "Keyboard shortcuts",
                    "setting.applicationHotkeys",
                    serde_json::json!({}),
                ),
            ] {
                if language == "en" {
                    continue;
                }
                assert_eq!(
                    text(language, caption),
                    translate_with(language, key, &options).unwrap(),
                    "{language}: {caption}"
                );
                assert!(!text(language, caption).contains("{{"));
            }
        }
        assert_eq!(
            text("en", "Autosave queue to server: true"),
            "Autosave queue to server: true"
        );
        assert_eq!(
            text("fr", "Autosave queue to server: true"),
            format!("{}: true", translate("fr", "setting.autosave").unwrap())
        );
    }

    #[test]
    fn native_context_labels_use_existing_catalog_entries_in_every_language() {
        for (label, key) in [
            ("Appearance", "page.setting.theme"),
            (
                "Background blur",
                "page.fullscreenPlayer.config.dynamicImageBlur",
            ),
            ("Change artwork", "common.edit"),
            ("Connect to a server", "form.addServer.title"),
            ("Create share link", "common.create"),
            ("Edit lyrics", "common.edit"),
            ("Enabled", "common.enable"),
            ("Export", "form.lyricsExport.export"),
            ("Images", "page.itemDetail.artworkTab"),
            ("Link type", "setting.discordLinkType"),
            ("Loading album tracks...", "dlna.speakerProperties.loading"),
            (
                "Loading artist information...",
                "dlna.speakerProperties.loading",
            ),
            (
                "Maximum sample rate (Hz, 0 = original)",
                "setting.transcodeSampleRate",
            ),
            (
                "Open application data folder",
                "action.openApplicationDirectory",
            ),
            ("Queue", "player.viewQueue"),
            ("Queue is empty", "common.noResultsFromQuery"),
            ("Remove override", "common.reset"),
            ("Reset equalizer", "common.resetToDefault"),
            ("Show albums", "page.genreList.showAlbums"),
            ("Show songs", "page.genreList.showTracks"),
            ("Use lyrics", "common.confirm"),
        ] {
            for language in languages().into_iter().filter(|language| *language != "en") {
                assert_eq!(
                    text(language, label),
                    caption(language, key).unwrap(),
                    "{language}: {label}"
                );
            }
        }
        // Settings save and library queue actions use explicit keys at their call sites.
        assert_eq!(
            text("fr", "common.save"),
            translate("fr", "common.save").unwrap()
        );
        assert_eq!(
            text("fr", "player.addLast"),
            translate("fr", "player.addLast").unwrap()
        );
        assert_eq!(text("fr", "Apply"), "Apply");
    }

    #[test]
    fn reuses_every_existing_translation_with_english_fallback() {
        assert!(languages().len() >= 30);
        assert_ne!(text("fr", "Home"), "Home");
        assert_eq!(text("fr", "home"), text("fr", "Home"));
        assert_eq!(
            text("fr", "Home: 42"),
            format!("{}: 42", text("fr", "Home"))
        );
        assert!(rtl("ar") && rtl("fa-IR"));
        assert!(!rtl("en") && !rtl("fr"));
        assert_eq!(text("unknown-language", "Home"), "Home");
        assert_eq!(
            text("fr", "Native label without an existing translation"),
            "Native label without an existing translation"
        );
        assert!(translate("es", "entity.album_other").is_some());
    }

    #[test]
    fn contextual_labels_and_plural_fallbacks_match_original_i18next() {
        use std::io::Read;
        let mut bytes = vec![];
        flate2::read::GzDecoder::new(
            include_bytes!("../assets/i18n-context-reference.json.gz").as_slice(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        let cases: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(cases.len(), 49_446);
        for case in cases {
            let key = case["key"].as_str().unwrap();
            assert_eq!(
                translate_with(case["language"].as_str().unwrap(), key, &case["options"])
                    .unwrap_or_else(|| key.into()),
                case["expected"].as_str().unwrap(),
                "{case}"
            );
        }
    }
}
