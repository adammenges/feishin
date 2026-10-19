//! Preserve the original JoinedArtists display text and longest nonoverlapping links.
use std::collections::HashSet;

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct Part {
    pub text: String,
    pub id: Option<String>,
}

pub fn listed(artists: Vec<(String, String)>, separator: &str) -> Vec<Part> {
    let mut parts = Vec::new();
    for (index, (id, text)) in artists.into_iter().enumerate() {
        if index > 0 {
            parts.push(Part {
                text: separator.into(),
                id: None,
            });
        }
        parts.push(Part {
            text,
            id: (!id.is_empty()).then_some(id),
        });
    }
    parts
}

pub fn joined(name: &str, artists: &[(String, String)], read_only: bool) -> Vec<Part> {
    let units = name.encode_utf16().collect::<Vec<_>>();
    let mut matches = Vec::new();
    for (index, (_, artist)) in artists.iter().enumerate() {
        if artist.is_empty() {
            continue;
        }
        let mut pattern = String::new();
        for ch in artist.chars() {
            if ".*+?^${}()|[]\\".contains(ch) {
                pattern.push('\\');
            }
            pattern.push(ch);
        }
        if let Some(regex) = crate::playback_filters::regex(&pattern) {
            matches.extend(
                regex
                    .find_from_ucs2(&units, 0)
                    .map(|found| (found.range, index)),
            );
        }
    }
    matches.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.start.cmp(&b.0.start)));
    let mut selected: Vec<(std::ops::Range<usize>, usize)> = Vec::new();
    for candidate in matches {
        if !selected
            .iter()
            .any(|(range, _)| candidate.0.start < range.end && candidate.0.end > range.start)
        {
            selected.push(candidate);
        }
    }
    selected.sort_by_key(|(range, _)| range.start);
    let mut parts = Vec::new();
    let add = |parts: &mut Vec<Part>, text: String, id: Option<&str>| {
        if !text.is_empty() {
            parts.push(Part {
                text,
                id: id
                    .filter(|id| !id.is_empty() && !read_only)
                    .map(str::to_owned),
            });
        }
    };
    if selected.is_empty() {
        if artists.is_empty() {
            add(&mut parts, name.into(), None);
        } else {
            for (index, (id, artist)) in artists.iter().enumerate() {
                if index > 0 {
                    add(&mut parts, ", ".into(), None);
                }
                add(&mut parts, artist.clone(), Some(id));
            }
        }
        return parts;
    }
    let mut last = 0;
    let mut matched_ids = HashSet::new();
    for (range, index) in selected {
        if range.start > last {
            add(
                &mut parts,
                String::from_utf16_lossy(&units[last..range.start]),
                None,
            );
        }
        let id = &artists[index].0;
        add(
            &mut parts,
            String::from_utf16_lossy(&units[range.clone()]),
            Some(id),
        );
        last = range.end;
        matched_ids.insert(id);
    }
    if last < units.len() {
        add(&mut parts, String::from_utf16_lossy(&units[last..]), None);
    }
    for (id, artist) in artists
        .iter()
        .filter(|(id, artist)| !artist.is_empty() && !matched_ids.contains(id))
    {
        add(&mut parts, ", ".into(), None);
        add(&mut parts, artist.clone(), Some(id));
    }
    parts
}

pub fn links(item: &crate::model::Item, album: bool) -> Vec<(String, String)> {
    let (saved, ids, names, id, name) = if album {
        (
            &item.album_artist_links,
            &item.album_artist_ids,
            &item.album_artist_names,
            &item.album_artist_id,
            &item.album_artist,
        )
    } else {
        (
            &item.artist_links,
            &item.artist_ids,
            &item.artist_names,
            &item.artist_id,
            &item.artist,
        )
    };
    if let Some(saved) = saved {
        return saved.clone();
    }
    // Older saved queues lack paired identities. Retain their single-artist fallback.
    if names.is_empty() {
        return if id.is_empty() && name.is_empty() {
            vec![]
        } else {
            vec![(id.clone(), name.clone())]
        };
    }
    names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                ids.get(index).cloned().unwrap_or_else(|| {
                    if index == 0 {
                        id.clone()
                    } else {
                        String::new()
                    }
                }),
                name.clone(),
            )
        })
        .collect()
}

pub fn contains(item: &crate::model::Item, artist_id: &str, album: bool) -> bool {
    let (paired, ids, primary) = if album {
        (
            &item.album_artist_links,
            &item.album_artist_ids,
            &item.album_artist_id,
        )
    } else {
        (&item.artist_links, &item.artist_ids, &item.artist_id)
    };
    if artist_id.is_empty() {
        return false;
    }
    paired.as_ref().map_or_else(
        || primary == artist_id || ids.iter().any(|id| id == artist_id),
        |links| links.iter().any(|(id, _)| id == artist_id),
    )
}

type Links = Vec<(String, String)>;
fn text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) => value.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn participants(
    value: &serde_json::Value,
    backend: crate::model::ServerType,
    kind: crate::model::ItemKind,
) -> indexmap::IndexMap<String, Links> {
    use crate::model::{ItemKind, ServerType};
    let mut result = indexmap::IndexMap::<String, Links>::new();
    let role_name = |role: &str, sub_role: &serde_json::Value| {
        sub_role
            .as_str()
            .filter(|sub| !sub.is_empty())
            .map_or_else(|| role.to_owned(), |sub| format!("{role} ({sub})"))
    };
    if backend == ServerType::Navidrome {
        for (role, artists) in value["participants"].as_object().into_iter().flatten() {
            if matches!(role.as_str(), "artist" | "albumartist")
                || role == "remixer" && kind == ItemKind::Album
            {
                continue;
            }
            if role == "remixer" {
                result.insert(
                    role.clone(),
                    artists
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|artist| (text(&artist["id"]), text(&artist["name"])))
                        .collect(),
                );
                continue;
            }
            let mut sub_roles: Vec<(Option<&serde_json::Value>, Links)> = vec![];
            for artist in artists.as_array().into_iter().flatten() {
                let sub_role = artist.get("subRole");
                let artist = (text(&artist["id"]), text(&artist["name"]));
                if let Some((_, artists)) = sub_roles.iter_mut().find(|(key, _)| *key == sub_role) {
                    artists.push(artist);
                } else {
                    sub_roles.push((sub_role, vec![artist]));
                }
            }
            // The original groups raw subroles first; absent/empty keys can then replace one another.
            for (sub_role, artists) in sub_roles {
                result.insert(
                    role_name(role, sub_role.unwrap_or(&serde_json::Value::Null)),
                    artists,
                );
            }
        }
    } else if backend == ServerType::Subsonic {
        for contributor in value["contributors"].as_array().into_iter().flatten() {
            let role = role_name(&text(&contributor["role"]), &contributor["subRole"]);
            let artist = &contributor["artist"];
            result
                .entry(role)
                .or_default()
                .push((text(&artist["id"]), text(&artist["name"])));
        }
    }
    result
}

pub(crate) fn normalize(
    value: &serde_json::Value,
    backend: crate::model::ServerType,
    kind: crate::model::ItemKind,
) -> (Links, Links) {
    use crate::model::{ItemKind, ServerType};
    use serde_json::Value;
    fn list(value: &Value, jellyfin: bool) -> Links {
        value
            .as_array()
            .into_iter()
            .flatten()
            .map(|entry| {
                (
                    text(&entry[if jellyfin { "Id" } else { "id" }]),
                    text(&entry[if jellyfin { "Name" } else { "name" }]),
                )
            })
            .collect()
    }
    fn append_remixers(artists: &mut Links, remixers: Links) {
        let existing = artists
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>();
        artists.extend(
            remixers
                .into_iter()
                .filter(|(id, _)| !existing.contains(id)),
        );
    }
    match backend {
        ServerType::Jellyfin => {
            let album = list(&value["AlbumArtists"], true);
            let artists = list(&value["ArtistItems"], true);
            (
                if artists.is_empty() {
                    album.clone()
                } else {
                    artists
                },
                album,
            )
        }
        ServerType::Navidrome => {
            let participants = &value["participants"];
            let album = if participants["albumartist"].is_array() {
                list(&participants["albumartist"], false)
            } else {
                vec![(text(&value["albumArtistId"]), text(&value["albumArtist"]))]
            };
            let mut artists = vec![];
            // The original album helper assigns both roles in server field order.
            for (role, entries) in participants.as_object().into_iter().flatten() {
                if role == "artist" || kind == ItemKind::Album && role == "remixer" {
                    artists = list(entries, false);
                }
            }
            if artists.is_empty()
                && !text(&value["artistId"]).is_empty()
                && !text(&value["artist"]).is_empty()
            {
                artists.push((text(&value["artistId"]), text(&value["artist"])));
            }
            if kind != ItemKind::Album {
                append_remixers(&mut artists, list(&participants["remixer"], false));
            }
            (artists, album)
        }
        ServerType::Subsonic => {
            let fallback = || vec![(text(&value["artistId"]), text(&value["artist"]))];
            let album_key = if kind == ItemKind::Album {
                "artists"
            } else {
                "albumArtists"
            };
            let album = if value[album_key].is_array() {
                list(&value[album_key], false)
            } else {
                fallback()
            };
            let mut artists = if value["artists"].is_array() {
                list(&value["artists"], false)
            } else if value["contributors"].is_array() {
                vec![]
            } else {
                fallback()
            };
            let remixers = value["contributors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|entry| {
                    entry["role"] == "remixer"
                        && entry["subRole"].as_str().is_none_or(str::is_empty)
                })
                .map(|entry| (text(&entry["artist"]["id"]), text(&entry["artist"]["name"])))
                .collect();
            append_remixers(&mut artists, remixers);
            (
                if kind == ItemKind::Album {
                    vec![]
                } else {
                    artists
                },
                album,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_prefers_normalized_identities_and_retains_saved_queue_fallbacks() {
        let mut song = crate::model::Item {
            artist_id: "legacy performer".into(),
            album_artist_ids: vec!["legacy album artist".into()],
            ..Default::default()
        };
        assert!(contains(&song, "legacy performer", false));
        assert!(contains(&song, "legacy album artist", true));
        assert!(!contains(&song, "", false));
        song.artist_links = Some(vec![("remixer".into(), "Remixer".into())]);
        song.album_artist_links = Some(vec![]);
        assert!(contains(&song, "remixer", false));
        assert!(!contains(&song, "legacy performer", false));
        assert!(!contains(&song, "legacy album artist", true));
    }
    #[test]
    fn original_card_artist_names_separators_and_links() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/joined-artists-reference.json")).unwrap();
        for case in reference["cardCases"].as_array().unwrap() {
            let artists = case["artists"]
                .as_array()
                .unwrap()
                .iter()
                .map(|artist| {
                    (
                        artist["id"].as_str().unwrap().into(),
                        artist["name"].as_str().unwrap().into(),
                    )
                })
                .collect();
            assert_eq!(
                serde_json::to_value(listed(artists, " • ")).unwrap(),
                case["parts"],
                "{case}"
            );
        }
    }
    #[test]
    fn original_participant_roles_and_composer_names() {
        use crate::{
            api::Api,
            model::{Item, ItemKind, Server, ServerType, Session},
        };
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/joined-artists-reference.json")).unwrap();
        for case in reference["participantCases"].as_array().unwrap() {
            let backend = match case["backend"].as_str().unwrap() {
                "Jellyfin" => ServerType::Jellyfin,
                "Navidrome" => ServerType::Navidrome,
                _ => ServerType::Subsonic,
            };
            let kind = if case["kind"] == "Album" {
                ItemKind::Album
            } else {
                ItemKind::Song
            };
            let api = Api::new(
                Server {
                    id: uuid::Uuid::new_v4(),
                    name: "Participants".into(),
                    url: "http://127.0.0.1:1".into(),
                    username: "fixture".into(),
                    kind: backend,
                    legacy_auth: false,
                    options: Default::default(),
                },
                Session::default(),
            )
            .unwrap();
            let item = api.normalize(&case["input"], kind);
            assert_eq!(
                serde_json::to_value(&item.participants).unwrap(),
                case["participants"],
                "{case}"
            );
            assert_eq!(
                serde_json::to_value(item.participants.keys().collect::<Vec<_>>()).unwrap(),
                case["roleOrder"],
                "{case}"
            );
            let composer = item
                .participants
                .get("composer")
                .into_iter()
                .flatten()
                .map(|(_, name)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            assert_eq!(crate::lists::text(&item, "composer", 0), composer);
            let parts = joined(
                "",
                item.participants.get("composer").map_or(&[], Vec::as_slice),
                false,
            );
            assert_eq!(
                serde_json::to_value(parts).unwrap(),
                case["composer"],
                "{case}"
            );
            let restored: Item =
                serde_json::from_slice(&serde_json::to_vec(&item).unwrap()).unwrap();
            assert_eq!(restored, item);
            assert_eq!(
                serde_json::to_value(restored.participants.keys().collect::<Vec<_>>()).unwrap(),
                case["roleOrder"]
            );
        }
    }
    #[test]
    fn original_backend_artist_pairs() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/joined-artists-reference.json")).unwrap();
        for case in reference["normalization"].as_array().unwrap() {
            let backend = match case["backend"].as_str().unwrap() {
                "Jellyfin" => crate::model::ServerType::Jellyfin,
                "Navidrome" => crate::model::ServerType::Navidrome,
                _ => crate::model::ServerType::Subsonic,
            };
            let kind = if case["kind"] == "Album" {
                crate::model::ItemKind::Album
            } else {
                crate::model::ItemKind::Song
            };
            let (artists, album) = normalize(&case["input"], backend, kind);
            assert_eq!(
                serde_json::to_value(artists).unwrap(),
                case["artists"],
                "{case}"
            );
            assert_eq!(
                serde_json::to_value(album).unwrap(),
                case["albumArtists"],
                "{case}"
            );
        }
    }
    #[test]
    fn original_inline_artist_text_and_links() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/joined-artists-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let artists = case["artists"]
                .as_array()
                .unwrap()
                .iter()
                .map(|artist| {
                    (
                        artist["id"].as_str().unwrap().into(),
                        artist["name"].as_str().unwrap().into(),
                    )
                })
                .collect::<Vec<_>>();
            let result = joined(
                case["artistName"].as_str().unwrap(),
                &artists,
                case["readOnly"].as_bool().unwrap(),
            );
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                case["parts"],
                "{case}"
            );
        }
    }
}
