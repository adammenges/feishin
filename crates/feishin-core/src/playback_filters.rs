//! Queue exclusions matching the Electron player's enabled, OR-combined rules.
use crate::{model::Item, storage::Settings};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FIELDS: &[(&str, &str)] = &[
    ("name", "Title"),
    ("albumArtist", "Album artist"),
    ("artist", "Artist"),
    ("duration", "Duration (milliseconds)"),
    ("genre", "Genre"),
    ("year", "Year"),
    ("releaseYear", "Release year"),
    ("note", "Note"),
    ("path", "Path"),
    ("playCount", "Play count"),
    ("favorite", "Favorite"),
    ("rating", "Rating"),
];
pub const OPERATORS: &[(&str, &str)] = &[
    ("is", "Is"),
    ("isNot", "Is not"),
    ("contains", "Contains"),
    ("notContains", "Does not contain"),
    ("startsWith", "Starts with"),
    ("endsWith", "Ends with"),
    ("regex", "Matches regex"),
    ("gt", "Is greater than"),
    ("lt", "Is less than"),
];
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    pub id: String,
    pub field: String,
    pub operator: String,
    pub value: Value,
    #[serde(default, rename = "isEnabled")]
    pub enabled: bool,
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            field: "name".into(),
            operator: "is".into(),
            value: Value::String(String::new()),
            enabled: true,
        }
    }
}
pub fn numeric(field: &str) -> bool {
    matches!(
        field,
        "duration" | "year" | "releaseYear" | "playCount" | "rating"
    )
}
pub fn operators(field: &str) -> Vec<(&'static str, &'static str)> {
    OPERATORS
        .iter()
        .copied()
        .filter(|(op, _)| {
            if numeric(field) {
                matches!(*op, "is" | "isNot" | "gt" | "lt")
            } else if field == "favorite" {
                matches!(*op, "is" | "isNot")
            } else {
                !matches!(*op, "gt" | "lt")
            }
        })
        .collect()
}
pub fn validate(filters: &[Filter]) -> Result<()> {
    ensure!(filters.len() <= 1000, "Too many playback filters");
    let mut ids = std::collections::HashSet::new();
    for filter in filters {
        ensure!(
            filter.id.len() <= 512 && ids.insert(&filter.id),
            "Invalid playback filter identity"
        );
        ensure!(
            FIELDS.iter().any(|(field, _)| *field == filter.field),
            "Invalid playback filter field"
        );
        // Older exports permit these operators although the player treats them as a match.
        ensure!(
            OPERATORS.iter().any(|(op, _)| *op == filter.operator)
                || matches!(
                    filter.operator.as_str(),
                    "inTheRange"
                        | "before"
                        | "after"
                        | "beforeDate"
                        | "afterDate"
                        | "inTheRangeDate"
                        | "inTheLast"
                        | "notInTheLast"
                ),
            "Invalid playback filter operator"
        );
        let scalar = |value: &Value| {
            value.is_boolean()
                || value.is_number()
                || value.as_str().is_some_and(|s| s.len() <= 65536)
        };
        ensure!(
            scalar(&filter.value)
                || filter.value.as_array().is_some_and(
                    |a| a.len() <= 100 && a.iter().all(|v| !v.is_boolean() && scalar(v))
                ),
            "Invalid playback filter value"
        );
    }
    Ok(())
}
fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Number(n) => n.as_f64().unwrap_or_default().to_string(),
        _ => value.to_string(),
    }
}
fn js_number(value: &Value) -> f64 {
    if let Some(b) = value.as_bool() {
        return if b { 1. } else { 0. };
    }
    let s = js_string(value);
    let s = s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if s.is_empty() {
        return 0.;
    }
    for (prefix, radix) in [("0x", 16), ("0b", 2), ("0o", 8)] {
        if s.to_ascii_lowercase().starts_with(prefix) {
            return u64::from_str_radix(&s[2..], radix).map_or(f64::NAN, |n| n as f64);
        }
    }
    s.parse().unwrap_or(f64::NAN)
}
fn field_value(item: &Item, field: &str, settings: &Settings) -> Option<Value> {
    Some(match field {
        "name" => item.name.clone().into(),
        "albumArtist" => item
            .album_artist_names
            .first()
            .unwrap_or(&item.album_artist)
            .clone()
            .into(),
        "artist" => if item.artist.is_empty() {
            item.primary_artist()
        } else {
            &item.artist
        }
        .into(),
        "duration" => (item.duration * 1000.).into(),
        "genre" => item
            .genre_names
            .first()
            .unwrap_or(&item.genre)
            .clone()
            .into(),
        "year" => item.year.into(),
        "releaseYear" => item
            .column_data
            .get("releaseYear")
            .and_then(|v| v.parse::<u32>().ok())
            .or_else(|| {
                item.column_data
                    .get("releaseDate")
                    .and_then(|v| v.get(..4))
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(item.year)
            .into(),
        "note" => item.comment.clone().into(),
        "path" => settings.resolve_song_path(&item.path).into(),
        "playCount" => item.play_count.into(),
        "favorite" => item.favorite.into(),
        "rating" => item.rating.into(),
        _ => return None,
    })
}
fn pattern_units(pattern: &str) -> Vec<u32> {
    let units = pattern.encode_utf16().collect::<Vec<_>>();
    let mut parsed = Vec::with_capacity(units.len());
    let mut index = 0;
    while index < units.len() {
        if units[index] == u16::from(b'\\') && index + 1 < units.len() {
            // regress joins escaped surrogate pairs even without JS's Unicode flag.
            // Raw surrogate units preserve legacy JS atoms and character classes.
            if units[index + 1] == u16::from(b'u')
                && index + 6 <= units.len()
                && let Ok(hex) = String::from_utf16(&units[index + 2..index + 6])
                && let Ok(unit) = u16::from_str_radix(&hex, 16)
                && (0xd800..=0xdfff).contains(&unit)
            {
                parsed.push(u32::from(unit));
                index += 6;
                continue;
            }
            parsed.push(u32::from(units[index]));
            index += 1;
        }
        parsed.push(u32::from(units[index]));
        index += 1;
    }
    parsed
}
/// Compile legacy JavaScript /i patterns using UTF-16 atoms and invalid-pattern fallback.
pub(crate) fn regex(pattern: &str) -> Option<regress::Regex> {
    if pattern.is_empty() {
        return None;
    }
    regress::Regex::from_unicode(pattern_units(pattern).into_iter(), "i").ok()
}
/// Keep order and duplicates; an enabled rule excludes a track if it matches.
pub fn apply(items: &mut Vec<Item>, settings: &Settings) -> usize {
    let rules = settings
        .playback_filters
        .iter()
        .filter(|f| f.enabled && !f.value.is_null() && f.value != "")
        .map(|filter| {
            let pattern = (filter.operator == "regex")
                .then(|| regex(&js_string(&filter.value)))
                .flatten();
            (filter, pattern)
        })
        .collect::<Vec<_>>();
    let before = items.len();
    if rules.is_empty() {
        return 0;
    }
    items.retain(|item| {
        !rules.iter().any(|(f, pattern)| {
            let Some(value) = field_value(item, &f.field, settings) else {
                return false;
            };
            let text = js_string(&value);
            let lower = text.to_lowercase();
            let expected = js_string(&f.value).to_lowercase();
            match f.operator.as_str() {
                "is" => lower == expected,
                "isNot" => lower != expected,
                "contains" => lower.contains(&expected),
                "notContains" => !lower.contains(&expected),
                "startsWith" => lower.starts_with(&expected),
                "endsWith" => lower.ends_with(&expected),
                "gt" => js_number(&value) > js_number(&f.value),
                "lt" => js_number(&value) < js_number(&f.value),
                "regex" => pattern.as_ref().is_some_and(|p| {
                    p.find_from_ucs2(&text.encode_utf16().collect::<Vec<_>>(), 0)
                        .next()
                        .is_some()
                }),
                _ => true,
            }
        })
    });
    before - items.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn matches_original_player_reference_cases() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/playback-filter-reference.json")).unwrap();
        let items: Vec<Item> = serde_json::from_value(reference["items"].clone()).unwrap();
        for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
            let settings = Settings {
                path_replace: "/music/".into(),
                path_replace_with: "/mnt/".into(),
                playback_filters: serde_json::from_value(case["filters"].clone()).unwrap(),
                ..Default::default()
            };
            let mut items = items.clone();
            apply(&mut items, &settings);
            assert_eq!(
                json!(items.iter().map(|i| &i.id).collect::<Vec<_>>()),
                case["expected"],
                "Reference {index}: {}",
                case["filters"]
            );
        }
    }
    #[test]
    fn exclusions_preserve_electron_values_operators_order_and_duplicates() {
        let base = Item {
            name: "Live live".into(),
            duration: 180.,
            favorite: false,
            genre_names: vec!["Jazz".into(), "Live".into()],
            artist: "One, Two".into(),
            album_artist_names: vec!["First".into(), "Second".into()],
            year: 2002,
            path: "/music/track.flac".into(),
            column_data: [("releaseYear".into(), "2024".into())].into(),
            ..Default::default()
        };
        let keep = Item {
            name: "Studio".into(),
            duration: 240.,
            favorite: true,
            ..base.clone()
        };
        for (field, op, value) in [
            ("name", "contains", json!("LIVE")),
            ("name", "regex", json!(r"^(live) \1$")),
            ("name", "regex", json!(r"(?<=Live )live$")),
            ("duration", "lt", json!("200000")),
            ("favorite", "is", json!(false)),
            ("favorite", "lt", json!(true)),
        ] {
            let settings = Settings {
                playback_filters: vec![Filter {
                    field: field.into(),
                    operator: op.into(),
                    value,
                    ..Default::default()
                }],
                ..Default::default()
            };
            let mut tracks = vec![base.clone(), keep.clone(), keep.clone(), base.clone()];
            assert_eq!(apply(&mut tracks, &settings), 2, "{field} {op}");
            assert_eq!(tracks, [keep.clone(), keep.clone()]);
        }
        for (field, value) in [
            ("genre", json!("Jazz")),
            ("albumArtist", json!("First")),
            ("artist", json!("One, Two")),
            ("releaseYear", json!(2024)),
            ("path", json!("/mnt/track.flac")),
        ] {
            let settings = Settings {
                path_replace: "/music/".into(),
                path_replace_with: "/mnt/".into(),
                playback_filters: vec![Filter {
                    field: field.into(),
                    value,
                    ..Default::default()
                }],
                ..Default::default()
            };
            assert_eq!(apply(&mut vec![base.clone()], &settings), 1, "{field}");
        }
        for (op, value, enabled) in [
            ("regex", json!("["), true),
            ("is", json!(""), true),
            ("contains", json!("Live"), false),
        ] {
            let settings = Settings {
                playback_filters: vec![Filter {
                    operator: op.into(),
                    value,
                    enabled,
                    ..Default::default()
                }],
                ..Default::default()
            };
            assert_eq!(apply(&mut vec![base.clone()], &settings), 0);
        }
        let emoji = Item {
            name: "😀".into(),
            ..base
        };
        let settings = Settings {
            playback_filters: vec![Filter {
                operator: "regex".into(),
                value: json!("^..$"),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(apply(&mut vec![emoji], &settings), 1);
    }
    #[test]
    fn imported_playback_filters_are_previewed_validated_and_round_trip() {
        let bytes = serde_json::to_vec(&json!({"playback":{"filters":[{"id":"first","field":"favorite","operator":"is","value":false,"isEnabled":true},{"id":"old","field":"duration","operator":"gt","value":0}]}})).unwrap();
        let preview = crate::migration::preview(&Settings::default(), &bytes).unwrap();
        assert!(
            preview
                .changes
                .iter()
                .any(|(key, _, _)| key == "playback_filters")
        );
        assert!(
            !preview
                .omitted
                .iter()
                .any(|key| key.starts_with("playback.filters"))
        );
        assert!(!preview.settings.playback_filters[1].enabled);
        let settings: Settings =
            serde_json::from_slice(&serde_json::to_vec(&preview.settings).unwrap()).unwrap();
        assert_eq!(settings.playback_filters, preview.settings.playback_filters);
        let mut filters = settings.playback_filters;
        filters[0].field = "password".into();
        assert!(validate(&filters).is_err());
    }
}
