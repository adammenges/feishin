use crate::{
    filters::{Filters, View},
    model::{ItemKind, Sort},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub kind: ItemKind,
    pub view: View,
}
pub fn validate(collections: &[Collection]) -> Result<()> {
    ensure!(collections.len() <= 1000, "Too many saved collections");
    let mut ids = BTreeSet::new();
    for collection in collections {
        ensure!(
            !collection.id.is_empty()
                && collection.id.len() <= 128
                && !collection.id.contains(['\0', '\n', '/', '?', '#'])
                && ids.insert(&collection.id),
            "Invalid or duplicate collection identity"
        );
        // Keep existing 256-character native names while accepting the original
        // UI's 128 UTF-16 code units without imposing a shorter UTF-8 byte limit.
        ensure!(
            !collection.name.trim().is_empty()
                && collection.name.encode_utf16().count() <= 256
                && !collection.name.chars().any(char::is_control),
            "Enter a collection name of 1 to 256 characters"
        );
        ensure!(
            matches!(collection.kind, ItemKind::Song | ItemKind::Album),
            "Collections contain songs or albums"
        );
        crate::filters::validate_views(&BTreeMap::from([(
            collection.id.clone(),
            collection.view.clone(),
        )]))?;
    }
    Ok(())
}
pub fn import_electron(value: &serde_json::Value) -> Result<Vec<Collection>> {
    let values = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Invalid Electron collections"))?;
    let mut collections = vec![];
    for value in values {
        let kind = match value["type"].as_str() {
            Some("album") => ItemKind::Album,
            Some("song") => ItemKind::Song,
            _ => anyhow::bail!("Invalid collection type"),
        };
        let query = value["filterQueryString"].as_str().unwrap_or_default();
        ensure!(query.len() <= 65536, "Collection query is too large");
        let params = url::form_urlencoded::parse(query.trim_start_matches('?').as_bytes())
            .into_owned()
            .collect::<Vec<_>>();
        let first = |key: &str| {
            params
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.as_str())
        };
        let list = |key: &str| {
            params
                .iter()
                .filter(|(name, value)| name == key && !value.is_empty())
                .map(|(_, value)| value.clone())
                .collect::<Vec<_>>()
        };
        let boolean = |key: &str| first(key).map(|value| value.parse::<bool>()).transpose();
        let year = |key: &str| first(key).map(|value| value.parse::<u32>()).transpose();
        let sort = match first("sortBy").unwrap_or("name") {
            "name" => Sort::Name,
            "album" => Sort::Album,
            "albumArtist" => Sort::AlbumArtist,
            "artist" => Sort::Artist,
            "bpm" => Sort::Bpm,
            "channels" => Sort::Channels,
            "comment" => Sort::Comment,
            "duration" => Sort::Duration,
            "explicitStatus" => Sort::Explicit,
            "favorited" => Sort::Favorited,
            "genre" => Sort::Genre,
            "id" => Sort::Id,
            "playCount" => Sort::Frequent,
            "random" => Sort::Random,
            "rating" => Sort::Rating,
            "recentlyAdded" => Sort::Newest,
            "recentlyPlayed" => Sort::Recent,
            "releaseDate" => Sort::ReleaseDate,
            "releaseYear" => Sort::ReleaseYear,
            "sortName" => Sort::SortName,
            "year" => Sort::Year,
            "songCount" => Sort::SongCount,
            "communityRating" => Sort::CommunityRating,
            "criticRating" => Sort::CriticRating,
            _ => anyhow::bail!("Unknown collection sort"),
        };
        let custom = first("_custom")
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or_default();
        let view = View {
            filters: Filters {
                favorite: boolean("favorite")?,
                has_rating: boolean("hasRating")?,
                compilation: boolean("compilation")?,
                recently_played: boolean("isRecentlyPlayed")?,
                min_year: year("minYear")?,
                max_year: year("maxYear")?,
                artist_ids: list("artistIds"),
                album_artist_ids: list("albumArtistIds"),
                genre_ids: list("genreIds"),
                library_ids: list("musicFolderId"),
                custom,
                ..Default::default()
            },
            text: first("searchTerm").unwrap_or_default().into(),
            sort,
            descending: first("sortOrder") == Some("DESC"),
            artist_role: None,
        };
        collections.push(Collection {
            id: value["id"].as_str().unwrap_or_default().into(),
            name: value["name"].as_str().unwrap_or_default().into(),
            kind,
            view,
        });
    }
    validate(&collections)?;
    Ok(collections)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_collections_import_repeated_ids_false_values_and_encoded_tags() {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("favorite", "false"),
                ("artistIds", "first"),
                ("artistIds", "second"),
                ("genreIds", "rock"),
                ("minYear", "1999"),
                ("maxYear", "2005"),
                ("searchTerm", "Live recording"),
                ("sortBy", "releaseYear"),
                ("sortOrder", "DESC"),
                ("_custom", r#"{"mood":["calm","happy"]}"#),
            ])
            .finish();
        let values = serde_json::json!([{"id":"existing-id","name":"Collection","type":"song","filterQueryString":query}]);
        let collections = import_electron(&values).unwrap();
        assert_eq!(collections[0].id, "existing-id");
        assert_eq!(collections[0].view.filters.artist_ids, ["first", "second"]);
        assert_eq!(collections[0].view.filters.favorite, Some(false));
        assert_eq!(
            collections[0].view.filters.custom["mood"],
            serde_json::json!(["calm", "happy"])
        );
        assert_eq!(collections[0].view.sort, Sort::ReleaseYear);
        assert!(collections[0].view.descending);
        let restored: Vec<Collection> =
            serde_json::from_str(&serde_json::to_string(&collections).unwrap()).unwrap();
        assert_eq!(restored, collections);
        assert!(validate(&[collections[0].clone(), collections[0].clone()]).is_err());
    }
}

#[cfg(test)]
#[test]
fn collection_names_preserve_original_unicode_input_and_existing_native_names() {
    for name in ["a".repeat(256), "界".repeat(128), "😀".repeat(64)] {
        let collection = Collection {
            id: "saved".into(),
            name,
            kind: ItemKind::Album,
            view: Default::default(),
        };
        validate(&[collection]).unwrap();
    }
    let oversized = Collection {
        id: "saved".into(),
        name: "界".repeat(257),
        kind: ItemKind::Album,
        view: Default::default(),
    };
    assert!(validate(&[oversized]).is_err());
}
