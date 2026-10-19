use crate::model::{Item, Page};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Filters {
    pub favorite: Option<bool>,
    pub has_rating: Option<bool>,
    pub compilation: Option<bool>,
    pub recently_played: Option<bool>,
    pub min_year: Option<u32>,
    pub max_year: Option<u32>,
    pub artist_ids: Vec<String>,
    pub artist_all: bool,
    pub album_artist_ids: Vec<String>,
    pub album_artist_all: bool,
    pub genre_ids: Vec<String>,
    pub genre_all: bool,
    pub library_ids: Vec<String>,
    pub custom: BTreeMap<String, Value>,
}
impl Filters {
    pub fn is_empty(&self) -> bool {
        self.favorite.is_none()
            && self.has_rating.is_none()
            && self.compilation.is_none()
            && self.recently_played.is_none()
            && self.min_year.is_none()
            && self.max_year.is_none()
            && self.artist_ids.is_empty()
            && self.album_artist_ids.is_empty()
            && self.genre_ids.is_empty()
            && self.library_ids.is_empty()
            && self.custom.is_empty()
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.min_year.is_none_or(|year| year <= 9999)
                && self.max_year.is_none_or(|year| year <= 9999)
                && self
                    .min_year
                    .zip(self.max_year)
                    .is_none_or(|(min, max)| min <= max),
            "Invalid year range"
        );
        for (ids, genre_names) in [
            (&self.artist_ids, false),
            (&self.album_artist_ids, false),
            (&self.genre_ids, true),
            (&self.library_ids, false),
        ] {
            ensure!(
                ids.len() <= 100
                    && ids.iter().all(|id| !id.is_empty()
                        && id.len() <= 512
                        && !id.contains(['\0', '\n', '\r'])
                        && (genre_names || !id.contains(['/', '?', '#']))),
                "Invalid filter selection"
            );
        }
        ensure!(self.custom.len() <= 100, "Too many custom filters");
        for (key, value) in &self.custom {
            ensure!(
                !key.is_empty()
                    && key.len() <= 128
                    && key
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                    && !key.starts_with('_')
                    && !matches!(
                        key.to_lowercase().as_str(),
                        "u" | "p"
                            | "t"
                            | "s"
                            | "c"
                            | "v"
                            | "f"
                            | "username"
                            | "password"
                            | "token"
                            | "apikey"
                            | "userid"
                            | "limit"
                            | "startindex"
                            | "recursive"
                    ),
                "Invalid custom filter name"
            );
            let valid = |value: &Value| match value {
                Value::String(text) => text.len() <= 4096 && !text.contains('\0'),
                Value::Bool(_) | Value::Number(_) => true,
                _ => false,
            };
            ensure!(
                valid(value)
                    || value
                        .as_array()
                        .is_some_and(|values| values.len() <= 100 && values.iter().all(valid)),
                "Invalid custom filter value"
            );
        }
        Ok(())
    }
    pub fn matches(&self, item: &Item) -> bool {
        let year = item.column_data.get("releaseYear").map_or_else(
            || (item.year > 0).then_some(item.year),
            |value| value.parse::<u32>().ok(),
        );
        self.favorite
            .is_none_or(|favorite| item.favorite == favorite)
            && self
                .has_rating
                .is_none_or(|rated| (item.rating > 0) == rated)
            && self
                .compilation
                .is_none_or(|compilation| item.is_compilation == compilation)
            && self
                .recently_played
                .is_none_or(|played| (item.play_count > 0) == played)
            && self
                .min_year
                .is_none_or(|min| year.is_some_and(|year| year >= min))
            && self
                .max_year
                .is_none_or(|max| year.is_some_and(|year| year <= max))
            && selected_match(&self.artist_ids, self.artist_all, |id| {
                item.artist_id == id || item.artist_ids.iter().any(|value| value == id)
            })
            && selected_match(&self.album_artist_ids, self.album_artist_all, |id| {
                item.album_artist_id == id || item.album_artist_ids.iter().any(|value| value == id)
            })
            && selected_match(&self.genre_ids, self.genre_all, |id| {
                item.genre_ids.iter().any(|value| value == id)
                    || item.genre.split(", ").any(|genre| genre == id)
            })
    }
    pub fn search(&self, items: Vec<Item>, query: &str, kind: crate::model::ItemKind) -> Vec<Item> {
        crate::lists::search_list(
            items
                .into_iter()
                .filter(|item| self.matches(item))
                .collect(),
            query,
            kind,
        )
    }
    pub fn page(&self, items: Vec<Item>, search: &str, offset: usize, limit: usize) -> Page {
        crate::model::filtered_page(
            items
                .into_iter()
                .filter(|item| self.matches(item))
                .collect(),
            search,
            offset,
            limit,
        )
    }
    pub fn custom_parameters(&self, comma_delimited: bool) -> Vec<(String, String)> {
        let text = |value: &Value| {
            value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string())
        };
        let mut parameters = vec![];
        for (key, value) in &self.custom {
            if let Some(values) = value.as_array() {
                if comma_delimited {
                    parameters.push((
                        key.clone(),
                        values.iter().map(text).collect::<Vec<_>>().join(","),
                    ));
                } else {
                    parameters.extend(values.iter().map(|value| (key.clone(), text(value))));
                }
            } else {
                parameters.push((key.clone(), text(value)));
            }
        }
        parameters
    }
}
fn selected_match(ids: &[String], all: bool, matches: impl Fn(&str) -> bool) -> bool {
    ids.is_empty()
        || if all {
            ids.iter().all(|id| matches(id))
        } else {
            ids.iter().any(|id| matches(id))
        }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct View {
    pub filters: Filters,
    pub text: String,
    pub sort: crate::model::Sort,
    pub descending: bool,
    pub artist_role: Option<String>,
}
pub fn validate_views(views: &BTreeMap<String, View>) -> Result<()> {
    ensure!(views.len() <= 5000, "Too many saved library views");
    for (key, view) in views {
        ensure!(
            !key.is_empty()
                && key.len() <= 512
                && view.text.len() <= 4096
                && !view.text.contains('\0')
                && view
                    .artist_role
                    .as_ref()
                    .is_none_or(|role| role.len() <= 100 && !role.contains(['\0', '\n'])),
            "Invalid saved library view"
        );
        view.filters.validate()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_preserve_duplicate_tracks_before_paging_and_validate_trust_boundaries() {
        let item = Item {
            id: "same".into(),
            year: 2000,
            favorite: true,
            rating: 3,
            genre_ids: vec!["rock".into()],
            ..Default::default()
        };
        let filters = Filters {
            favorite: Some(true),
            has_rating: Some(true),
            min_year: Some(1990),
            max_year: Some(2001),
            genre_ids: vec!["rock".into()],
            custom: BTreeMap::from([("mood".into(), serde_json::json!(["one", "two"]))]),
            ..Default::default()
        };
        filters.validate().unwrap();
        let page = filters.page(vec![Item::default(), item.clone(), item], "", 1, 1);
        assert_eq!(page.total, Some(2));
        assert_eq!(page.items[0].id, "same");
        assert_eq!(
            filters.custom_parameters(false),
            [("mood".into(), "one".into()), ("mood".into(), "two".into())]
        );
        assert_eq!(
            filters.custom_parameters(true),
            [("mood".into(), "one,two".into())]
        );
        let item = Item {
            artist_ids: vec!["a".into()],
            album_artist_ids: vec!["b".into()],
            genre_ids: vec!["rock".into(), "pop".into()],
            year: 2000,
            column_data: BTreeMap::from([("releaseYear".into(), "2005".into())]),
            ..Default::default()
        };
        let mut selection = Filters {
            artist_ids: vec!["a".into(), "c".into()],
            ..Default::default()
        };
        assert!(selection.matches(&item));
        selection.artist_all = true;
        assert!(!selection.matches(&item));
        selection.artist_ids.pop();
        selection.album_artist_ids = vec!["b".into()];
        selection.genre_ids = vec!["rock".into(), "pop".into()];
        selection.genre_all = true;
        selection.min_year = Some(2004);
        assert!(selection.matches(&item));
        selection.genre_ids.push("jazz".into());
        assert!(!selection.matches(&item));
        for key in ["_start", "UserId", "apiKey", "password", "x&token"] {
            let filters = Filters {
                custom: BTreeMap::from([(key.into(), true.into())]),
                ..Default::default()
            };
            assert!(filters.validate().is_err());
        }
    }
}
