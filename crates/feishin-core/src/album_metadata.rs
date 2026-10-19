use crate::{
    lists,
    model::{Item, ItemKind, ServerType},
    releases::Setting,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ops::Range};

fn partial_date(value: &serde_json::Value, datetime: bool) -> Option<(String, u32)> {
    let raw = value.as_str()?;
    let text = raw.trim();
    if datetime && !matches!(text.len(), 4 | 7 | 10) {
        return partial_date(&serde_json::json!(raw.get(..10)?), false);
    }
    let parts = text.split('-').collect::<Vec<_>>();
    if !(1..=3).contains(&parts.len())
        || parts[0].len() != 4
        || parts[1..].iter().any(|part| part.len() != 2)
        || parts
            .iter()
            .any(|part| !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some((text.into(), parts[0].parse().ok()?))
}

pub(crate) fn normalize_details(item: &mut Item, value: &serde_json::Value, server: ServerType) {
    use serde_json::{Value, json};
    let array = |value: &Value| value.as_array().cloned().unwrap_or_default();
    let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
    if let Some(subtitle) = value["discSubtitle"]
        .as_str()
        .filter(|text| !text.is_empty())
    {
        item.column_data
            .insert("discSubtitle".into(), subtitle.into());
    }
    if server == ServerType::Jellyfin {
        item.tags = array(&value["Tags"])
            .iter()
            .filter_map(|tag| tag.as_str().map(|tag| (tag.into(), vec![])))
            .collect();
    } else if let Some(tags) = value["tags"].as_object() {
        item.tags = tags
            .iter()
            .map(|(name, values)| {
                (
                    name.clone(),
                    array(values)
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect(),
                )
            })
            .collect();
    }
    if item.kind == ItemKind::Song
        && server == ServerType::Navidrome
        && value["tags"]["subtitle"].is_array()
    {
        item.column_data
            .insert("trackSubtitle".into(), item.tags["subtitle"].join(" · "));
    }
    if item.kind == ItemKind::Song {
        normalize_song_dates(item, value, server);
    }
    if item.kind != ItemKind::Album {
        return;
    }
    let release_type = match server {
        ServerType::Navidrome => value["mbzAlbumType"].as_str(),
        ServerType::Subsonic => value["releaseTypes"].as_array().and_then(|types| {
            types
                .iter()
                .filter_map(Value::as_str)
                .find(|kind| matches!(*kind, "album" | "ep" | "single" | "broadcast" | "other"))
        }),
        ServerType::Jellyfin => None,
    };
    item.column_data.insert(
        "releaseType".into(),
        release_type.unwrap_or_default().into(),
    );
    let year = |field: &str| {
        value[field]
            .as_u64()
            .filter(|year| *year > 0 && *year <= 9999)
            .map(|year| (year.to_string(), year as u32))
    };
    let (release, original) = match server {
        ServerType::Navidrome => {
            let release = partial_date(&value["releaseDate"], false)
                .or_else(|| partial_date(&value["date"], false))
                .or_else(|| year("minYear"));
            let original = partial_date(&value["originalDate"], false)
                .or_else(|| partial_date(&value["releaseDate"], false))
                .or_else(|| partial_date(&value["date"], false))
                .or_else(|| {
                    year(if value["minOriginalYear"].is_null() {
                        "minYear"
                    } else {
                        "minOriginalYear"
                    })
                });
            item.record_labels = item.tags.remove("recordlabel").unwrap_or_default();
            if let Some(version) = item
                .tags
                .remove("albumversion")
                .filter(|values| !values.is_empty())
            {
                item.column_data
                    .insert("version".into(), version.join(" · "));
            }
            item.tags.remove("genre");
            item.tags.remove("releasetype");
            item.column_data.insert(
                "trackYearRange".into(),
                json!({"min":value["minYear"],"max":value["maxYear"]}).to_string(),
            );
            if value["discs"].is_object() {
                item.column_data
                    .insert("discs".into(), value["discs"].to_string());
            }
            if let Some(cover) = value["coverArtId"]
                .as_str()
                .filter(|cover| !cover.is_empty())
            {
                item.cover_id = cover.into();
            }
            (release, original)
        }
        ServerType::Subsonic => {
            let date = &value["releaseDate"];
            let release = if let (Some(y), Some(m), Some(d)) = (
                date["year"].as_u64(),
                date["month"].as_u64(),
                date["day"].as_u64(),
            ) {
                partial_date(&json!(format!("{y}-{m:02}-{d:02}")), false)
                    .or_else(|| Some((String::new(), 0)))
            } else {
                year("year")
            };
            item.record_labels = array(&value["recordLabels"])
                .iter()
                .map(|label| text(&label["name"]))
                .filter(|label| !label.is_empty())
                .collect();
            let discs = array(&value["discTitles"])
                .iter()
                .filter_map(|disc| {
                    Some((
                        disc["disc"].as_u64()?.to_string(),
                        disc["title"].as_str()?.to_owned(),
                    ))
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            if !discs.is_empty() {
                item.column_data
                    .insert("discs".into(), json!(discs).to_string());
            }
            (release.clone(), release)
        }
        ServerType::Jellyfin => {
            let release = partial_date(&value["PremiereDate"], true)
                .map(|(date, y)| {
                    (
                        date,
                        if y == 0 {
                            year("ProductionYear").map_or(0, |(_, y)| y)
                        } else {
                            y
                        },
                    )
                })
                .or_else(|| year("ProductionYear"));
            item.record_labels = array(&value["Studios"])
                .iter()
                .map(|label| text(&label["Name"]))
                .filter(|label| !label.is_empty())
                .collect();
            (release.clone(), release)
        }
    };
    for (field, date) in [("releaseDate", &release), ("originalDate", &original)] {
        item.column_data.remove(field);
        item.column_data.remove(if field == "releaseDate" {
            "releaseYear"
        } else {
            "originalYear"
        });
        if let Some((date, year)) = date {
            item.column_data.insert(field.into(), date.clone());
            if *year > 0 || server != ServerType::Jellyfin {
                item.column_data.insert(
                    if field == "releaseDate" {
                        "releaseYear".into()
                    } else {
                        "originalYear".into()
                    },
                    year.to_string(),
                );
            }
        }
    }
    item.year = release.map_or(0, |(_, year)| year);
}

fn normalize_song_dates(item: &mut Item, value: &serde_json::Value, server: ServerType) {
    let year = |key: &str| {
        value[key]
            .as_u64()
            .filter(|year| *year > 0 && *year <= 9999)
            .map(|year| year as u32)
    };
    let from_year = |year: Option<u32>| year.map(|year| (year.to_string(), year));
    let (date, track_year, release, original, original_year) = match server {
        ServerType::Navidrome => {
            let date = partial_date(&value["date"], false).or_else(|| from_year(year("year")));
            let track_year = year("year").or_else(|| {
                date.as_ref()
                    .map(|(_, year)| *year)
                    .filter(|year| *year > 0)
            });
            let release_date = partial_date(&value["releaseDate"], false);
            let release = if let Some(year) = year("releaseYear") {
                Some((
                    release_date.map_or_else(|| year.to_string(), |(date, _)| date),
                    year,
                ))
            } else {
                release_date.or_else(|| {
                    let date = partial_date(&value["date"], false);
                    year("year")
                        .map(|year| {
                            (
                                date.as_ref()
                                    .map_or_else(|| year.to_string(), |(date, _)| date.clone()),
                                year,
                            )
                        })
                        .or(date)
                })
            };
            let original = partial_date(&value["originalDate"], false)
                .or_else(|| from_year(year("originalYear")));
            let original_year = year("originalYear").or_else(|| {
                original
                    .as_ref()
                    .map(|(_, year)| *year)
                    .filter(|year| *year > 0)
            });
            (date, track_year, release, original, original_year)
        }
        ServerType::Subsonic => {
            let date = from_year(year("year"));
            let rd = &value["releaseDate"];
            let release = if let (Some(y), Some(m), Some(d)) = (
                rd["year"].as_u64(),
                rd["month"].as_u64(),
                rd["day"].as_u64(),
            ) {
                partial_date(&serde_json::json!(format!("{y}-{m:02}-{d:02}")), false)
                    .or_else(|| Some((String::new(), 0)))
            } else {
                date.clone()
            };
            (date.clone(), year("year"), release, date, year("year"))
        }
        ServerType::Jellyfin => {
            let premiere = partial_date(&value["PremiereDate"], true);
            let release_year = premiere
                .as_ref()
                .map(|(_, year)| *year)
                .filter(|year| *year > 0)
                .or_else(|| year("ProductionYear"));
            let release = premiere
                .map(|(date, _)| (date, release_year.unwrap_or_default()))
                .or_else(|| from_year(release_year));
            (
                release.clone(),
                release_year,
                release.clone(),
                release,
                release_year,
            )
        }
    };
    item.year = track_year.unwrap_or_default();
    for (key, value) in [
        ("date", date.map(|(date, _)| date)),
        ("year", track_year.map(|year| year.to_string())),
        (
            "releaseDate",
            release.as_ref().map(|(date, _)| date.clone()),
        ),
        (
            "releaseYear",
            release
                .map(|(_, year)| year)
                .filter(|year| *year > 0 || server != ServerType::Jellyfin)
                .map(|year| year.to_string()),
        ),
        ("originalDate", original.map(|(date, _)| date)),
        ("originalYear", original_year.map(|year| year.to_string())),
    ] {
        item.column_data
            .insert(key.into(), value.unwrap_or_default());
    }
}

pub const FIELDS: [(&str, &str); 8] = [
    ("albumArtists", "Album artists"),
    ("releaseDate", "Release date"),
    ("releaseYear", "Release year"),
    ("songCount", "Songs"),
    ("duration", "Duration"),
    ("releaseType", "Release type"),
    ("genres", "Genres"),
    ("size", "File size"),
];
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub image_size: f32,
    pub show_favorite_rating: bool,
    pub vertical_layout: bool,
    pub items: Vec<Setting>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            image_size: 0.,
            show_favorite_rating: true,
            vertical_layout: true,
            items: FIELDS
                .iter()
                .map(|(id, _)| Setting {
                    id: (*id).into(),
                    disabled: *id != "albumArtists",
                })
                .collect(),
        }
    }
}
impl Config {
    pub fn validate(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.image_size.is_finite() && (0. ..=2000.).contains(&self.image_size),
            "Album group image size must be from 0 to 2000 pixels"
        );
        let mut seen = HashSet::new();
        anyhow::ensure!(
            self.items
                .iter()
                .all(|item| FIELDS.iter().any(|(id, _)| *id == item.id)
                    && seen.insert(item.id.clone())),
            "Invalid album group metadata fields"
        );
        for entry in Self::default().items {
            if seen.insert(entry.id.clone()) {
                self.items.push(entry);
            }
        }
        Ok(())
    }
    pub fn artwork_size(&self) -> f32 {
        if self.image_size == 0. {
            96.
        } else {
            self.image_size.clamp(1., 2000.)
        }
    }
    pub fn rows(&self, album: &Item, songs: &[Item], language: &str) -> Vec<(String, String)> {
        let mut aggregate = songs.first().unwrap_or(album).clone();
        if !songs.is_empty() {
            aggregate.duration = songs.iter().map(|song| song.duration).sum();
            aggregate
                .column_data
                .insert("songCount".into(), songs.len().to_string());
            let size = songs
                .iter()
                .filter_map(|song| song.column_data.get("size")?.parse::<u64>().ok())
                .fold(0_u64, u64::saturating_add);
            aggregate
                .column_data
                .insert("size".into(), size.to_string());
            let genres = crate::genres::grouped(songs);
            aggregate.genre = genres
                .iter()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            aggregate.genre_links = Some(genres);
        }
        self.items
            .iter()
            .filter(|item| !item.disabled)
            .filter_map(|item| {
                let value = match item.id.as_str() {
                    "albumArtists" => crate::artists::joined(
                        &aggregate.album_artist,
                        &crate::artists::links(&aggregate, true),
                        true,
                    )
                    .into_iter()
                    .map(|part| part.text)
                    .collect::<String>(),
                    "duration" => {
                        if aggregate.duration > 0. {
                            crate::metadata_format::duration_text(
                                aggregate.duration,
                                language,
                                false,
                            )
                        } else {
                            String::new()
                        }
                    }
                    "releaseType" => aggregate
                        .tags
                        .get("releasetype")
                        .and_then(|types| {
                            crate::releases::metadata_labels(types, language)
                                .into_iter()
                                .next()
                        })
                        .unwrap_or_default(),
                    "releaseDate" => crate::metadata_format::date(
                        aggregate
                            .column_data
                            .get("releaseDate")
                            .map(String::as_str)
                            .unwrap_or(""),
                        language,
                    ),
                    "releaseYear" => aggregate
                        .column_data
                        .get("releaseYear")
                        .filter(|year| year.parse::<u32>().is_ok_and(|year| year > 0))
                        .cloned()
                        .unwrap_or_default(),
                    "songCount" => aggregate
                        .column_data
                        .get("songCount")
                        .and_then(|count| count.parse::<u64>().ok())
                        .filter(|count| *count > 0)
                        .map(|count| {
                            crate::metadata_format::count_text("songCount", count, language)
                        })
                        .unwrap_or_default(),
                    "size"
                        if aggregate
                            .column_data
                            .get("size")
                            .is_none_or(|value| value == "0") =>
                    {
                        String::new()
                    }
                    "size" => aggregate
                        .column_data
                        .get("size")
                        .and_then(|size| size.parse().ok())
                        .map(crate::metadata_format::size)
                        .unwrap_or_default(),
                    _ => lists::text(&aggregate, &item.id, 0),
                };
                (!value.is_empty()).then(|| (item.id.clone(), value))
            })
            .collect()
    }
}
/// Preserve list/playlist order and duplicate rows while combining adjacent albums.
pub fn groups(songs: &[Item]) -> Vec<Range<usize>> {
    let mut groups = vec![];
    let mut start = 0;
    for index in 1..=songs.len() {
        if index == songs.len()
            || songs[index].album.is_empty()
            || songs[index - 1].album.is_empty()
            || songs[index].album != songs[index - 1].album
            || songs[index].server_id != songs[index - 1].server_id
        {
            groups.push(start..index);
            start = index;
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn track_subtitles_match_original_backend_normalizers_and_survive_queue_storage() {
        use crate::{
            api::Api,
            model::{Server, Session},
        };
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/track-subtitles-reference.json")).unwrap();
        assert_eq!(cases.len(), 24);
        for case in cases {
            let api = Api::new(
                Server {
                    id: uuid::Uuid::new_v4(),
                    name: "Subtitle fixture".into(),
                    url: "http://127.0.0.1:1".into(),
                    username: "fixture".into(),
                    kind: serde_json::from_value(case["backend"].clone()).unwrap(),
                    legacy_auth: false,
                    options: Default::default(),
                },
                Session::default(),
            )
            .unwrap();
            let item = api.normalize(&case["input"], ItemKind::Song);
            assert_eq!(
                serde_json::to_value(item.column_data.get("trackSubtitle")).unwrap(),
                case["subtitle"],
                "{case}"
            );
            let mut queue = crate::queue::Queue::default();
            queue.add(vec![item.clone()], crate::queue::Enqueue::Now);
            let restored: crate::queue::Queue =
                serde_json::from_slice(&serde_json::to_vec(&queue).unwrap()).unwrap();
            assert_eq!(restored.current().unwrap().item, item);
            assert!(
                !api.normalize(&case["input"], ItemKind::Album)
                    .column_data
                    .contains_key("trackSubtitle")
            );
        }
    }
    #[test]
    fn album_metadata_matches_original_backend_normalizers() {
        use crate::{
            api::Api,
            model::{Server, Session},
        };
        use serde_json::Value;
        let cases: Vec<Value> =
            serde_json::from_slice(include_bytes!("../assets/album-metadata-reference.json"))
                .unwrap();
        assert_eq!(cases.len(), 24);
        for case in cases {
            let kind: ServerType = serde_json::from_value(case["kind"].clone()).unwrap();
            let api = Api::new(
                Server {
                    id: uuid::Uuid::new_v4(),
                    name: "Album fixture".into(),
                    url: "http://127.0.0.1:1".into(),
                    username: "fixture".into(),
                    kind,
                    legacy_auth: false,
                    options: Default::default(),
                },
                Session::default(),
            )
            .unwrap();
            let item = api.normalize(&case["input"], ItemKind::Album);
            let expected = &case["expected"];
            assert_eq!(
                item.year,
                expected["releaseYear"].as_u64().unwrap_or_default() as u32,
                "{kind:?} case {} displayed release year",
                case["index"]
            );
            for field in ["releaseDate", "originalDate", "version", "releaseType"] {
                assert_eq!(
                    item.column_data
                        .get(field)
                        .map(String::as_str)
                        .unwrap_or_default(),
                    expected[field].as_str().unwrap_or_default(),
                    "{kind:?} case {} {field}",
                    case["index"]
                );
            }
            for field in ["releaseYear", "originalYear"] {
                assert_eq!(
                    item.column_data
                        .get(field)
                        .and_then(|value| value.parse::<u32>().ok()),
                    expected[field].as_u64().map(|value| value as u32),
                    "{kind:?} case {} {field}",
                    case["index"]
                );
            }
            for field in ["discs", "trackYearRange"] {
                let actual: Value = item
                    .column_data
                    .get(field)
                    .map(|value| serde_json::from_str(value).unwrap())
                    .unwrap_or(Value::Null);
                assert_eq!(
                    actual, expected[field],
                    "{kind:?} case {} {field}",
                    case["index"]
                );
            }
            assert_eq!(
                serde_json::to_value(&item.record_labels).unwrap(),
                expected["recordLabels"]
            );
            assert_eq!(
                serde_json::to_value(&item.tags).unwrap(),
                if expected["tags"].is_null() {
                    serde_json::json!({})
                } else {
                    expected["tags"].clone()
                }
            );
        }
    }
    #[test]
    fn album_groups_preserve_duplicates_order_and_aggregate_visible_metadata() {
        let mut songs = ["A", "A", "B", "A", "", ""]
            .into_iter()
            .enumerate()
            .map(|(index, album)| Item {
                id: index.to_string(),
                album: album.into(),
                album_artist: "Artist".into(),
                duration: 35.,
                genre: if index == 0 {
                    "Jazz, Soul"
                } else {
                    "Jazz, Rock"
                }
                .into(),
                column_data: std::collections::BTreeMap::from([("size".into(), "1024".into())]),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        songs[1].id = songs[0].id.clone();
        assert_eq!(groups(&songs), [0..2, 2..3, 3..4, 4..5, 5..6]);
        assert!(groups(&[]).is_empty());
        let mut config = Config::default();
        for field in &mut config.items {
            field.disabled = false;
        }
        config.items.swap(0, 3);
        let rows = config.rows(&songs[0], &songs[..2], "en");
        assert_eq!(rows[0], ("songCount".into(), "2 tracks".into()));
        assert!(rows.contains(&("duration".into(), "1m 10s".into())));
        assert!(rows.contains(&("genres".into(), "Jazz, Soul, Rock".into())));
        assert!(rows.contains(&("size".into(), "2.00 KiB".into())));
        config.validate().unwrap();
        config.items.push(config.items[0].clone());
        assert!(config.validate().is_err());
    }
}
