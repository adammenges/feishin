use crate::{
    i18n, metadata_format,
    model::{Item, ItemKind},
};

pub type Field = (&'static str, &'static str, u32);

pub(crate) fn normalize(
    item: &mut Item,
    value: &serde_json::Value,
    backend: crate::model::ServerType,
) {
    use crate::model::ServerType;
    use serde_json::Value;
    if !matches!(
        item.kind,
        ItemKind::Song | ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
    ) {
        return;
    }
    let artist = matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist);
    let null = Value::Null;
    let rating = match backend {
        ServerType::Navidrome => &value["rating"],
        ServerType::Subsonic if !artist => &value["userRating"],
        _ => &null,
    };
    item.rating = rating
        .as_u64()
        .and_then(|value| value.try_into().ok())
        .unwrap_or_default();
    let rating = if item.rating == 0 { &null } else { rating };
    let last_played = if backend == ServerType::Navidrome
        && value["playDate"]
            .as_str()
            .is_some_and(|date| !date.is_empty() && !date.contains("0001-"))
    {
        &value["playDate"]
    } else {
        &null
    };
    let created = if artist {
        &null
    } else {
        match backend {
            ServerType::Navidrome => &value["createdAt"],
            ServerType::Subsonic => &value["created"],
            ServerType::Jellyfin => &value["DateCreated"],
        }
    };
    let updated = if artist {
        &null
    } else {
        match backend {
            ServerType::Navidrome => &value["updatedAt"],
            ServerType::Subsonic if item.kind == ItemKind::Album => &value["created"],
            ServerType::Subsonic => &null,
            ServerType::Jellyfin
                if item.kind == ItemKind::Album
                    && value["DateLastMediaAdded"]
                        .as_str()
                        .is_some_and(|date| !date.is_empty()) =>
            {
                &value["DateLastMediaAdded"]
            }
            ServerType::Jellyfin => &value["DateCreated"],
        }
    };
    for (key, value) in [
        ("userRating", rating),
        ("lastPlayedAt", last_played),
        ("createdAt", created),
        ("updatedAt", updated),
    ] {
        item.column_data.insert(
            key.into(),
            match value {
                Value::String(value) => value.clone(),
                Value::Number(value) => value.to_string(),
                _ => String::new(),
            },
        );
    }
    if backend == ServerType::Subsonic && item.kind != ItemKind::Song {
        item.play_count = 0;
        item.column_data.insert("playCount".into(), String::new());
    }
    if item.kind == ItemKind::Album {
        let duration = match backend {
            ServerType::Jellyfin => value["RunTimeTicks"]
                .as_f64()
                .map(|ticks| ticks / 10_000_000.),
            _ => value["duration"].as_f64(),
        };
        item.column_data.insert(
            "duration".into(),
            duration.map_or_else(String::new, |duration| duration.to_string()),
        );
        if backend == ServerType::Jellyfin && value["ChildCount"].as_u64().unwrap_or_default() == 0
        {
            item.column_data.insert("songCount".into(), String::new());
        }
    }
    if artist || (item.kind == ItemKind::Album && backend != ServerType::Navidrome) {
        item.column_data.insert("size".into(), String::new());
    }
}

pub fn comment_html(text: &str) -> String {
    fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }
    static URLS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let urls = URLS.get_or_init(|| regex::Regex::new(r"((?:https?://)?(?:[A-Za-z0-9_-]{1,32}(?:\.[A-Za-z0-9_-]{1,32})+)(?:/[A-Za-z0-9_\-./?%&=][^.|^\t\n\x0b\x0c\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]*)?)").unwrap());
    let mut result = String::new();
    let mut end = 0;
    for link in urls.find_iter(text) {
        result.push_str(&escape(&text[end..link.start()]));
        let prefix = if link.as_str().starts_with("http") {
            ""
        } else {
            "https://"
        };
        result.push_str(&format!(
            "<a href=\"{prefix}{}\">{}</a>",
            escape(link.as_str()),
            escape(link.as_str())
        ));
        end = link.end();
    }
    result.push_str(&escape(&text[end..]));
    result
}

pub fn fields(kind: ItemKind) -> &'static [Field] {
    match kind {
        ItemKind::Song => &[
            ("name", "common.title", 1),
            ("path", "common.path", 1),
            ("albumArtists", "entity.albumArtist", 1),
            ("artists", "entity.artist", 2),
            ("album", "entity.album", 1),
            ("discNumber", "common.disc", 1),
            ("trackNumber", "common.trackNumber", 1),
            ("date", "filter.date", 1),
            ("year", "filter.year", 1),
            ("releaseDate", "filter.releaseDate", 1),
            ("releaseYear", "filter.releaseYear", 1),
            ("explicitStatus", "filter.explicitStatus", 1),
            ("genres", "entity.genre", 2),
            ("duration", "common.duration", 1),
            ("isCompilation", "filter.isCompilation", 1),
            ("container", "common.codec", 1),
            ("bitRate", "common.bitrate", 1),
            ("sampleRate", "common.sampleRate", 1),
            ("bitDepth", "common.bitDepth", 1),
            ("channels", "common.channel", 2),
            ("size", "common.size", 1),
            ("userFavorite", "common.favorite", 1),
            ("userRating", "common.rating", 1),
            ("playCount", "filter.playCount", 1),
            ("lastPlayedAt", "filter.lastPlayed", 1),
            ("updatedAt", "common.modified", 1),
            ("gainAlbum", "common.albumGain", 1),
            ("gainTrack", "common.trackGain", 1),
            ("peakAlbum", "common.albumPeak", 1),
            ("peakTrack", "common.trackPeak", 1),
            ("comment", "filter.comment", 1),
            ("id", "filter.id", 1),
        ],
        ItemKind::Album => &[
            ("name", "common.title", 1),
            ("albumArtists", "entity.albumArtist", 1),
            ("releaseType", "common.releaseType", 1),
            ("genres", "entity.genre", 2),
            ("duration", "common.duration", 1),
            ("releaseYear", "filter.releaseYear", 1),
            ("songCount", "filter.songCount", 1),
            ("explicitStatus", "filter.explicitStatus", 1),
            ("isCompilation", "filter.isCompilation", 1),
            ("size", "common.size", 1),
            ("userFavorite", "common.favorite", 1),
            ("userRating", "common.rating", 1),
            ("playCount", "filter.playCount", 1),
            ("lastPlayedAt", "filter.lastPlayed", 1),
            ("updatedAt", "common.modified", 1),
            ("comment", "filter.comment", 1),
            ("mbid", "common.mbid", 1),
            ("id", "filter.id", 1),
            ("version", "common.version", 1),
            ("recordLabels", "common.recordLabel", 1),
        ],
        ItemKind::Artist | ItemKind::AlbumArtist => &[
            ("name", "common.name", 1),
            ("genres", "entity.genre", 2),
            ("duration", "common.duration", 1),
            ("songCount", "filter.songCount", 1),
            ("userFavorite", "common.favorite", 1),
            ("userRating", "common.rating", 1),
            ("playCount", "filter.playCount", 1),
            ("lastPlayedAt", "filter.lastPlayed", 1),
            ("mbid", "common.mbid", 1),
            ("biography", "common.biography", 1),
            ("id", "filter.id", 1),
        ],
        ItemKind::Playlist => &[
            ("name", "common.title", 1),
            ("description", "common.description", 1),
            ("genres", "entity.genre", 2),
            ("duration", "common.duration", 1),
            ("songCount", "filter.songCount", 1),
            ("size", "common.size", 1),
            ("username", "common.owner", 1),
            ("public", "form.createPlaylist.input_public", 1),
            ("smart", "entity.smartPlaylist", 1),
            ("id", "filter.id", 1),
        ],
        _ => &[],
    }
}

pub fn label(field: &Field, language: &str) -> String {
    let value = i18n::translate_with(language, field.1, &serde_json::json!({"count":field.2}))
        .unwrap_or_else(|| field.1.into());
    if field.1 == "common.mbid" {
        return value;
    }
    value
        .split(". ")
        .map(|sentence| {
            let mut chars = sentence.chars();
            chars.next().map_or_else(String::new, |first| {
                format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
            })
        })
        .collect::<Vec<_>>()
        .join(". ")
}

pub fn text(item: &Item, id: &str, language: &str) -> String {
    match id {
        "artists" | "albumArtists" => crate::artists::links(item, id == "albumArtists")
            .into_iter()
            .map(|(_, name)| if name.is_empty() { "-".into() } else { name })
            .collect::<Vec<_>>()
            .join(" • "),
        "genres" => crate::genres::links(item)
            .into_iter()
            .map(|(_, name)| name)
            .collect::<Vec<_>>()
            .join(" • "),
        "album" if item.album_id.is_empty() => String::new(),
        "isCompilation" => item.is_compilation.to_string(),
        "userFavorite" => item.favorite.to_string(),
        "playCount"
            if item
                .column_data
                .get(id)
                .is_some_and(|value| value.is_empty()) =>
        {
            String::new()
        }
        "userRating" => {
            if item
                .column_data
                .get(id)
                .is_some_and(|value| value.is_empty())
            {
                String::new()
            } else {
                item.rating.to_string()
            }
        }
        "explicitStatus" => match item.explicit_status() {
            Some("EXPLICIT") => i18n::translate(language, "common.explicit").unwrap_or_default(),
            Some("CLEAN") => i18n::translate(language, "common.clean").unwrap_or_default(),
            _ => String::new(),
        },
        "releaseType" => {
            crate::releases::metadata_labels(&item.release_types, language).join(" • ")
        }
        "recordLabels" => item.record_labels.join(" • "),
        "mbid" => item.musicbrainz_id.clone(),
        "description" => item.comment.clone(),
        "biography" => crate::lyric_sources::text_from_html(&item.comment),
        "smart" => {
            if item.rules.is_some() {
                "true".into()
            } else {
                String::new()
            }
        }
        "gainAlbum" | "gainTrack" => (if id == "gainAlbum" {
            item.gain_album
        } else {
            item.gain_track
        })
        .map(|value| format!("{value} dB"))
        .unwrap_or_default(),
        "peakAlbum" | "peakTrack" => (if id == "peakAlbum" {
            item.peak_album
        } else {
            item.peak_track
        })
        .map(|value| value.to_string())
        .unwrap_or_default(),
        "discNumber" => item
            .column_data
            .get(id)
            .cloned()
            .unwrap_or_else(|| item.disc.to_string()),
        "trackNumber" => item.track.to_string(),
        _ => metadata_format::information_text(item, id, 0, language),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[test]
    fn information_normalization_matches_original_backends_and_live_rating_changes() {
        use crate::{
            api::Api,
            model::{Server, ServerType, Session},
        };
        let cases: Value = serde_json::from_str(include_str!(
            "../assets/information-normalize-reference.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let backend: ServerType = serde_json::from_value(case["kind"].clone()).unwrap();
            let api = Api::new(
                Server {
                    id: uuid::Uuid::nil(),
                    name: "Information fixture".into(),
                    url: "http://127.0.0.1:1".into(),
                    username: "fixture".into(),
                    kind: backend,
                    legacy_auth: false,
                    options: Default::default(),
                },
                Session::default(),
            )
            .unwrap();
            let kind = match case["itemKind"].as_str().unwrap() {
                "song" => ItemKind::Song,
                "album" => ItemKind::Album,
                "albumArtist" => ItemKind::AlbumArtist,
                _ => unreachable!(),
            };
            let mut item = api.normalize(&case["input"], kind);
            for (key, expected) in case["expected"].as_object().unwrap() {
                let actual = item.column_data.get(key).cloned().unwrap_or_else(|| {
                    if key == "playCount" {
                        item.play_count.to_string()
                    } else {
                        String::new()
                    }
                });
                let expected = match expected {
                    Value::Null => String::new(),
                    Value::String(value) => value.clone(),
                    _ => expected.to_string(),
                };
                assert_eq!(actual, expected, "{key}: {case}");
            }
            item.set_rating(4);
            assert_eq!(text(&item, "userRating", "en"), "4");
            item.set_rating(0);
            assert_eq!(text(&item, "userRating", "en"), "0");
        }
    }
    #[test]
    fn comment_links_match_original_and_keep_markup_literal() {
        let source: Value =
            serde_json::from_str(include_str!("../assets/metadata-surfaces-reference.json"))
                .unwrap();
        for case in source["comments"].as_array().unwrap() {
            let html = comment_html(case["input"].as_str().unwrap());
            let spans = crate::lyrics_markup::parse(&html);
            let links = spans
                .iter()
                .filter_map(|span| {
                    span.href
                        .as_ref()
                        .map(|href| serde_json::json!([span.text, href]))
                })
                .collect::<Vec<_>>();
            assert_eq!(serde_json::json!(links), case["links"], "{case}");
            assert!(!html.contains("<script>"));
        }
    }
    #[test]
    fn complete_information_fields_match_original_components() {
        let source: Value =
            serde_json::from_str(include_str!("../assets/metadata-surfaces-reference.json"))
                .unwrap();
        for case in source["fullInformation"].as_array().unwrap() {
            let row = &case["row"];
            let string = |key| row[key].as_str().unwrap_or_default().to_owned();
            let number = |key| row[key].as_u64().unwrap_or_default() as u32;
            let array =
                |key| serde_json::from_value::<Vec<String>>(row[key].clone()).unwrap_or_default();
            let links = |key| {
                row[key].as_array().map(|values| {
                    values
                        .iter()
                        .map(|value| {
                            (
                                value["id"].as_str().unwrap_or_default().into(),
                                value["name"].as_str().unwrap_or_default().into(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            };
            let item = Item {
                kind: serde_json::from_value(case["kind"].clone()).unwrap(),
                id: string("id"),
                name: string("name"),
                album: string("album"),
                album_id: string("albumId"),
                path: string("path"),
                container: string("container"),
                owner_name: string("owner"),
                comment: if case["kind"] == "AlbumArtist" {
                    string("biography")
                } else if case["kind"] == "Playlist" {
                    string("description")
                } else {
                    string("comment")
                },
                musicbrainz_id: if case["kind"] == "Album" {
                    string("mbzId")
                } else {
                    string("mbz")
                },
                year: number("year"),
                track: number("trackNumber"),
                disc: number("discNumber"),
                play_count: number("playCount"),
                duration: row["duration"].as_f64().unwrap_or_default() / 1000.,
                rating: number("userRating") as u8,
                favorite: row["userFavorite"].as_bool().unwrap_or_default(),
                is_compilation: row["isCompilation"].as_bool().unwrap_or_default(),
                public: row["public"].as_bool().unwrap_or_default(),
                release_types: array("releaseTypes"),
                record_labels: array("recordLabels"),
                rules: (!row["rules"].is_null()).then(|| row["rules"].clone()),
                artist_links: links("artists"),
                album_artist_links: links("albumArtists"),
                genre_links: links("genres"),
                gain_album: row["gain"]["album"].as_f64().map(|v| v as f32),
                gain_track: row["gain"]["track"].as_f64().map(|v| v as f32),
                peak_album: row["peak"]["album"].as_f64().map(|v| v as f32),
                peak_track: row["peak"]["track"].as_f64().map(|v| v as f32),
                column_data: row
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            match value {
                                Value::Null => String::new(),
                                Value::String(value) => value.clone(),
                                _ => value.to_string(),
                            },
                        )
                    })
                    .collect(),
                ..Default::default()
            };
            let language = case["language"].as_str().unwrap();
            let actual = fields(item.kind)
                .iter()
                .filter_map(|field| {
                    let value = text(&item, field.0, language);
                    (!value.is_empty()).then(|| {
                        serde_json::json!([field.1, field.2, value, label(field, language)])
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::json!(actual),
                case["fields"],
                "{} {} {}",
                case["kind"],
                language,
                row["id"]
            );
        }
    }
}
