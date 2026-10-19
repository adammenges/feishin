//! Genre identities and the original seeded badge palette.
use crate::model::{Item, ServerType};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

pub(crate) fn normalize(value: &Value, backend: ServerType) -> Vec<(String, String)> {
    let jellyfin = backend == ServerType::Jellyfin;
    let entries = &value[if jellyfin { "GenreItems" } else { "genres" }];
    if backend == ServerType::Subsonic && !entries.is_array() {
        return value["genre"]
            .as_str()
            .filter(|name| !name.is_empty())
            .map(|name| vec![(name.into(), name.into())])
            .unwrap_or_default();
    }
    entries
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| {
            let name = entry[if jellyfin { "Name" } else { "name" }]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let id = if backend == ServerType::Subsonic {
                name.clone()
            } else {
                match &entry[if jellyfin { "Id" } else { "id" }] {
                    Value::String(id) => id.clone(),
                    Value::Number(id) => id.to_string(),
                    _ => String::new(),
                }
            };
            (id, name)
        })
        .collect()
}

pub fn links(item: &Item) -> Vec<(String, String)> {
    if let Some(genres) = &item.genre_links {
        return genres.clone();
    }
    let names = if item.genre_names.is_empty() && !item.genre.is_empty() {
        vec![item.genre.clone()]
    } else {
        item.genre_names.clone()
    };
    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| (item.genre_ids.get(index).cloned().unwrap_or_default(), name))
        .collect()
}

/// Album groups retain the first genre for each ID in song order.
pub fn grouped(songs: &[Item]) -> Vec<(String, String)> {
    let mut genres = Vec::<(String, String)>::new();
    for song in songs {
        let values = if song.genre_links.is_none() && song.genre_names.is_empty() {
            // Older queues only stored a comma-separated display value.
            song.genre
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(|name| (name.to_owned(), name.to_owned()))
                .collect()
        } else {
            links(song)
        };
        for genre in values {
            if !genres.iter().any(|(id, _)| id == &genre.0) {
                genres.push(genre);
            }
        }
    }
    genres
}

/// Matches string-to-color 2.2.2 with the application's fixed seed, including color-name mixing.
pub fn badge_color(name: &str) -> ([u8; 3], bool) {
    #[derive(Deserialize)]
    struct Palette {
        pattern: String,
        names: BTreeMap<String, [u8; 3]>,
    }
    static PALETTE: OnceLock<(regress::Regex, BTreeMap<String, [u8; 3]>)> = OnceLock::new();
    let (words, names) = PALETTE.get_or_init(|| {
        let palette: Palette = serde_json::from_str(include_str!("../assets/genre-colors.json"))
            .expect("Bundled genre palette");
        (
            regress::Regex::new(&palette.pattern).expect("Bundled word pattern"),
            palette.names,
        )
    });
    let text = format!(
        "{{\"seed\":\"121212\",\"string\":{}}}",
        serde_json::to_string(name).unwrap()
    );
    let units = text.encode_utf16().collect::<Vec<_>>();
    let mut mixed = [0_u64; 3];
    let mut count = 0_u64;
    // JSON punctuation always selects lodash.words' Unicode-word branch.
    for word in words.find_from_ucs2(&units, 0) {
        let word = String::from_utf16_lossy(&units[word.range])
            .trim()
            .to_lowercase();
        if let Some(rgb) = names.get(&word) {
            for index in 0..3 {
                mixed[index] += u64::from(rgb[index]);
            }
            count += 1;
        }
    }
    let (mut hash, mut maximum) = (1_u64, 0_u64);
    const SEED: u64 = 16_777_215;
    for unit in &units {
        let unit = u64::from(*unit);
        maximum = maximum.max(unit);
        hash = (hash + unit * (SEED / maximum) * 49_979_693) % SEED;
    }
    let hex = format!("{:x}", (hash * units.len() as u64) % SEED);
    let hex = hex.repeat(6_usize.div_ceil(hex.len()));
    let mut rgb = [0; 3];
    for index in 0..3 {
        rgb[index] = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        if count > 0 {
            rgb[index] =
                (0.25 * f64::from(rgb[index]) + 0.75 * (mixed[index] as f64 / count as f64)) as u8;
        }
    }
    let linear = rgb.map(|channel| {
        let value = f64::from(channel) / 255.;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    let light = 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2] > 0.179;
    (rgb, light)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn genre_identities_and_colors_match_original_sources() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/genres-reference.json")).unwrap();
        for case in reference["normalization"].as_array().unwrap() {
            let backend = serde_json::from_value(case["backend"].clone()).unwrap();
            assert_eq!(
                serde_json::to_value(normalize(&case["input"], backend)).unwrap(),
                case["genres"],
                "{case}"
            );
            let api = crate::api::Api::new(
                crate::model::Server {
                    id: uuid::Uuid::new_v4(),
                    name: "Genres".into(),
                    url: "http://127.0.0.1:1".into(),
                    username: "fixture".into(),
                    kind: backend,
                    legacy_auth: false,
                    options: Default::default(),
                },
                crate::model::Session::default(),
            )
            .unwrap();
            for kind in [crate::model::ItemKind::Song, crate::model::ItemKind::Album] {
                let item = api.normalize(&case["input"], kind);
                assert_eq!(
                    serde_json::to_value(links(&item)).unwrap(),
                    case["genres"],
                    "{case}"
                );
                let restored: Item =
                    serde_json::from_value(serde_json::to_value(&item).unwrap()).unwrap();
                assert_eq!(links(&restored), links(&item));
                let text = links(&item)
                    .into_iter()
                    .map(|(_, name)| name)
                    .collect::<Vec<_>>()
                    .join(", ");
                assert_eq!(crate::lists::text(&restored, "genres", 0), text);
            }
        }
        for case in reference["groupCases"].as_array().unwrap() {
            let songs = case["songs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|song| Item {
                    genre_links: Some(normalize(song, ServerType::Navidrome)),
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::to_value(grouped(&songs)).unwrap(),
                case["genres"],
                "{case}"
            );
            let mut config = crate::album_metadata::Config::default();
            for field in &mut config.items {
                field.disabled = field.id != "genres";
            }
            let rows = config.rows(&Item::default(), &songs, "en");
            assert_eq!(
                rows.first()
                    .map(|(_, text)| text.as_str())
                    .unwrap_or_default(),
                case["text"].as_str().unwrap(),
                "{case}"
            );
        }
        for case in reference["colors"].as_array().unwrap() {
            let (rgb, light) = badge_color(case["name"].as_str().unwrap());
            assert_eq!(
                format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]),
                case["color"],
                "{case}"
            );
            assert_eq!(light, case["isLight"], "{case}");
        }
    }
}
