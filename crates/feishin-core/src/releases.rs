use crate::model::Item;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const TYPES: [(&str, &str, &str); 18] = [
    ("appears-on", "appearsOn", "Appears on"),
    ("album", "releaseTypeAlbum", "Albums"),
    ("audio drama", "releaseTypeAudioDrama", "Audio drama"),
    ("audiobook", "releaseTypeAudiobook", "Audiobooks"),
    ("broadcast", "releaseTypeBroadcast", "Broadcasts"),
    ("compilation", "releaseTypeCompilation", "Compilations"),
    ("demo", "releaseTypeDemo", "Demos"),
    ("dj-mix", "releaseTypeDjMix", "DJ mixes"),
    ("ep", "releaseTypeEp", "EPs"),
    (
        "field recording",
        "releaseTypeFieldRecording",
        "Field recordings",
    ),
    ("interview", "releaseTypeInterview", "Interviews"),
    ("live", "releaseTypeLive", "Live"),
    ("mixtape/street", "releaseTypeMixtapeStreet", "Mixtapes"),
    ("other", "releaseTypeOther", "Other"),
    ("remix", "releaseTypeRemix", "Remixes"),
    ("single", "releaseTypeSingle", "Singles"),
    ("soundtrack", "releaseTypeSoundtrack", "Soundtracks"),
    ("spokenword", "releaseTypeSpokenWord", "Spoken word"),
];
const PRIMARY: [&str; 5] = ["album", "broadcast", "ep", "other", "single"];
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Setting {
    pub id: String,
    pub disabled: bool,
}
pub fn defaults() -> Vec<Setting> {
    TYPES
        .iter()
        .map(|(id, _, _)| Setting {
            id: (*id).into(),
            disabled: false,
        })
        .collect()
}
pub fn label(key: &str) -> String {
    key.split('/')
        .map(|key| {
            TYPES
                .iter()
                .find(|(id, _, _)| *id == key)
                .map_or_else(|| key.to_owned(), |(_, _, label)| (*label).into())
        })
        .collect::<Vec<_>>()
        .join(" · ")
}
pub fn metadata_labels(types: &[String], language: &str) -> Vec<String> {
    let mut values = types
        .iter()
        .map(|value| {
            let lower = value.to_lowercase();
            let (group, key) = if PRIMARY.contains(&lower.as_str()) {
                (0, Some(lower.as_str()))
            } else {
                (
                    1,
                    match lower.as_str() {
                        "audiobook" | "compilation" | "demo" | "interview" | "live" | "remix"
                        | "soundtrack" => Some(lower.as_str()),
                        "audio drama" => Some("audioDrama"),
                        "dj-mix" => Some("djMix"),
                        "field recording" => Some("fieldRecording"),
                        "mixtape/street" => Some("mixtape"),
                        "spokenword" => Some("spokenWord"),
                        _ => None,
                    },
                )
            };
            if let Some(key) = key {
                let key = format!(
                    "releaseType.{}.{key}",
                    if group == 0 { "primary" } else { "secondary" }
                );
                (group, crate::i18n::translate(language, &key).unwrap_or(key))
            } else {
                (2, crate::i18n::title_case(value))
            }
        })
        .collect::<Vec<_>>();
    values.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.encode_utf16().cmp(b.1.encode_utf16()))
    });
    values.into_iter().map(|(_, value)| value).collect()
}
pub fn section_label(key: &str, language: &str) -> String {
    key.split('/')
        .map(|kind| match kind {
            "appears-on" => crate::i18n::translate(language, "page.albumArtistDetail.appearsOn")
                .unwrap_or_else(|| "Appears on".into()),
            "ep" => metadata_labels(&[kind.into()], language)
                .remove(0)
                .to_uppercase(),
            _ => metadata_labels(&[kind.into()], language).remove(0),
        })
        .collect::<Vec<_>>()
        .join(" • ")
}
pub fn from_electron(settings: &[Setting]) -> Vec<Setting> {
    settings
        .iter()
        .map(|setting| Setting {
            id: TYPES
                .iter()
                .find(|(_, legacy, _)| *legacy == setting.id)
                .map_or_else(|| setting.id.clone(), |(id, _, _)| (*id).into()),
            disabled: setting.disabled,
        })
        .collect()
}
pub fn validate(settings: &[Setting]) -> anyhow::Result<()> {
    let mut ids = BTreeSet::new();
    anyhow::ensure!(
        settings.len() <= 100
            && settings.iter().all(|setting| !setting.id.is_empty()
                && setting.id.len() <= 256
                && ids.insert(&setting.id)),
        "Invalid release group configuration"
    );
    Ok(())
}
fn key(album: &Item, artist: &str, combined: bool) -> String {
    if !crate::artists::contains(album, artist, true) {
        return "appears-on".into();
    }
    if combined && album.is_compilation {
        return "compilation".into();
    }
    let mut seen = BTreeSet::new();
    let mut types = album
        .release_types
        .iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty() && seen.insert(value.clone()))
        .collect::<Vec<_>>();
    if types.is_empty() {
        return "album".into();
    }
    if combined {
        types.sort_by_key(|kind| (!PRIMARY.contains(&kind.as_str()), kind.clone()));
        types.join("/")
    } else {
        ["album", "single", "ep", "broadcast", "other"]
            .into_iter()
            .find(|primary| types.iter().any(|kind| kind == primary))
            .map_or_else(|| types[0].clone(), str::to_owned)
    }
}
pub fn groups(
    items: &[Item],
    artist: &str,
    combined: bool,
    settings: &[Setting],
) -> Vec<(String, Vec<usize>)> {
    let mut grouped: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        grouped
            .entry(key(item, artist, combined))
            .or_default()
            .push(index);
    }
    let enabled = |kind: &str| {
        !TYPES.iter().any(|(id, _, _)| *id == kind)
            || settings
                .iter()
                .find(|setting| setting.id == kind)
                .is_some_and(|setting| !setting.disabled)
    };
    let priority = |kind: &str| {
        settings
            .iter()
            .filter(|setting| !setting.disabled)
            .position(|setting| setting.id == kind)
            .unwrap_or(999)
    };
    let group_priority = |key: &str| {
        let types = key.split('/').collect::<Vec<_>>();
        types
            .iter()
            .find(|kind| PRIMARY.contains(kind))
            .map_or_else(
                || types.iter().map(|kind| priority(kind)).min().unwrap_or(999),
                |kind| priority(kind),
            )
    };
    let secondary = |key: &str| {
        let mut values = key
            .split('/')
            .filter(|kind| !PRIMARY.contains(kind))
            .map(priority)
            .filter(|priority| *priority != 999)
            .collect::<Vec<_>>();
        values.sort();
        values
    };
    let mut groups = grouped
        .into_iter()
        .filter(|(key, _)| key.split('/').any(enabled))
        .collect::<Vec<_>>();
    // The original object retains first-seen group order when its comparator ties.
    groups.sort_by_key(|(_, indices)| indices[0]);
    let compare = |a: &(String, Vec<usize>), b: &(String, Vec<usize>)| {
        group_priority(&a.0)
            .cmp(&group_priority(&b.0))
            .then_with(|| {
                let (left, right) = (secondary(&a.0), secondary(&b.0));
                if a.0.contains('/') && b.0.contains('/') && !left.is_empty() && !right.is_empty() {
                    left.cmp(&right)
                } else {
                    crate::i18n::compare(&a.0, &b.0, false)
                }
            })
    };
    // The original comparator is not transitive for custom types without a saved priority.
    // Preserve its natural run and stable insertion order for these small group lists.
    // ponytail: quadratic moves are bounded by release-group count, not album count.
    let mut run = usize::from(!groups.is_empty());
    if groups.len() > 1 {
        let descending = compare(&groups[1], &groups[0]).is_lt();
        run = 2;
        while run < groups.len() && compare(&groups[run], &groups[run - 1]).is_lt() == descending {
            run += 1;
        }
        if descending {
            groups[..run].reverse();
        }
    }
    for index in run..groups.len() {
        let mut left = 0;
        let mut right = index;
        while left < right {
            let middle = (left + right) / 2;
            if compare(&groups[index], &groups[middle]).is_lt() {
                right = middle;
            } else {
                left = middle + 1;
            }
        }
        groups[left..=index].rotate_right(1);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn artist_groups_and_labels_match_original_preferences_and_translations() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/artist-album-groups-reference.json"))
                .unwrap();
        let items: Vec<Item> = serde_json::from_value(reference["items"].clone()).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let settings: Vec<Setting> = serde_json::from_value(case["settings"].clone()).unwrap();
            let actual = groups(
                &items,
                "artist",
                case["combined"].as_bool().unwrap(),
                &settings,
            )
            .into_iter()
            .map(|(key, indices)| {
                (
                    key,
                    indices
                        .into_iter()
                        .map(|index| items[index].id.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
            assert_eq!(serde_json::json!(actual), case["groups"], "{case}");
        }
        for label in reference["labels"].as_array().unwrap() {
            assert_eq!(
                section_label(
                    label["key"].as_str().unwrap(),
                    label["language"].as_str().unwrap()
                ),
                label["label"].as_str().unwrap(),
                "{label}"
            );
        }
    }
    #[test]
    fn discography_groups_preserve_artist_membership_compilations_combined_types_and_preferences() {
        let album = |types: &[&str]| Item {
            album_artist_ids: vec!["artist".into()],
            release_types: types.iter().map(|value| (*value).into()).collect(),
            ..Default::default()
        };
        let items = vec![
            album(&[" Live ", "ALBUM", "album"]),
            Item {
                album_artist_ids: vec!["other".into()],
                ..album(&["single"])
            },
            Item {
                is_compilation: true,
                ..album(&["album"])
            },
            album(&["ep"]),
            album(&["unrecognized"]),
        ];
        let mut settings = defaults();
        let grouped = groups(&items, "artist", true, &settings);
        assert_eq!(
            grouped
                .iter()
                .map(|(key, _)| key.as_str())
                .collect::<Vec<_>>(),
            [
                "appears-on",
                "album/live",
                "compilation",
                "ep",
                "unrecognized"
            ]
        );
        assert_eq!(
            groups(&items, "artist", false, &settings)
                .iter()
                .find(|(key, _)| key == "album")
                .unwrap()
                .1,
            [0, 2]
        );
        settings
            .iter_mut()
            .find(|setting| setting.id == "album")
            .unwrap()
            .disabled = true;
        assert!(
            groups(&items, "artist", true, &settings)
                .iter()
                .any(|(key, _)| key == "album/live")
        );
        settings
            .iter_mut()
            .find(|setting| setting.id == "live")
            .unwrap()
            .disabled = true;
        assert!(
            !groups(&items, "artist", true, &settings)
                .iter()
                .any(|(key, _)| key == "album/live")
        );
        let ep = settings
            .iter()
            .position(|setting| setting.id == "ep")
            .unwrap();
        let setting = settings.remove(ep);
        settings.insert(0, setting);
        assert_eq!(groups(&items, "artist", true, &settings)[0].0, "ep");
        assert_eq!(
            from_electron(&[Setting {
                id: "releaseTypeEp".into(),
                disabled: true
            }])[0]
                .id,
            "ep"
        );
    }
}
