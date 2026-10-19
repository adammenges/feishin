use crate::model::Item;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const FIELDS: &[(&str, &str, &str)] = &[
    ("title", "Title", "name"),
    ("artist", "Artist", "artists"),
    ("album", "Album", "album"),
    ("bit_depth", "Bit depth", "bitDepth"),
    ("bit_rate", "Bitrate", "bitRate"),
    ("bpm", "BPM", "bpm"),
    ("codec", "Codec", "container"),
    ("date", "Date", "date"),
    ("disc_number", "Disc", "discNumber"),
    ("genres", "Genres", "genres"),
    ("release_date", "Release date", "releaseDate"),
    ("release_type", "Release type", "releaseType"),
    ("release_year", "Release year", "releaseYear"),
    ("sample_rate", "Sample rate", "sampleRate"),
    ("track_number", "Track", "trackNumber"),
    ("year", "Year", "year"),
];
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub id: String,
    pub disabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub active_tab: String,
    pub expanded: bool,
    pub visualizer_expanded: bool,
    pub cover_art_size: f32,
    pub dynamic_background: bool,
    pub dynamic_is_image: bool,
    pub dynamic_image_blur: f32,
    pub opacity: f32,
    pub player_item_alignment: String,
    pub title_display_type: String,
    pub title_line_count: usize,
    pub use_image_aspect_ratio: bool,
    pub fields: Vec<Field>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            active_tab: String::new(),
            expanded: false,
            visualizer_expanded: false,
            cover_art_size: 75.,
            dynamic_background: true,
            dynamic_is_image: false,
            dynamic_image_blur: 6.,
            opacity: 25.,
            player_item_alignment: "center".into(),
            title_display_type: "scroll".into(),
            title_line_count: 1,
            use_image_aspect_ratio: false,
            fields: FIELDS
                .iter()
                .enumerate()
                .map(|(i, (id, _, _))| Field {
                    id: (*id).into(),
                    disabled: i > 2,
                })
                .collect(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (50. ..=100.).contains(&self.cover_art_size)
                && (0. ..=6.).contains(&self.dynamic_image_blur)
                && (0. ..=100.).contains(&self.opacity),
            "Invalid player artwork settings"
        );
        ensure!(
            matches!(
                self.player_item_alignment.as_str(),
                "left" | "center" | "right"
            ) && matches!(self.title_display_type.as_str(), "scroll" | "multiLine")
                && (1..=10).contains(&self.title_line_count),
            "Invalid player text settings"
        );
        ensure!(
            matches!(
                self.active_tab.as_str(),
                "" | "lyrics" | "queue" | "related" | "visualizer"
            ),
            "Invalid player tab"
        );
        let mut ids = HashSet::new();
        ensure!(
            self.fields.len() <= FIELDS.len()
                && self.fields.iter().all(|field| ids.insert(&field.id)
                    && FIELDS.iter().any(|(id, _, _)| *id == field.id)),
            "Invalid player metadata fields"
        );
        Ok(())
    }
    pub fn visible(&self, id: &str) -> bool {
        self.fields
            .iter()
            .any(|field| field.id == id && !field.disabled)
    }
    pub fn metadata(&self, item: &Item, language: &str) -> Vec<(&'static str, String)> {
        let has_metadata = self
            .fields
            .iter()
            .filter(|field| !field.disabled)
            .any(|field| {
                let Some((_, _, column)) = FIELDS.iter().find(|(id, _, _)| *id == field.id) else {
                    return false;
                };
                match *column {
                    "name" | "artists" | "album" => false,
                    // The original React expression treats an empty genre array or
                    // release-type badge as present, even when it paints no text.
                    "genres" => {
                        item.genre_links.is_some() || !crate::genres::links(item).is_empty()
                    }
                    "releaseType" => item.tags.contains_key("releasetype"),
                    "discNumber" => item.disc != 0,
                    "trackNumber" => item.track != 0,
                    "year" => item.year != 0,
                    "container" => !item.container.is_empty(),
                    _ => item
                        .column_data
                        .get(*column)
                        .is_some_and(|value| !value.is_empty() && value != "0"),
                }
            });
        self.fields
            .iter()
            .filter(|field| !field.disabled)
            .filter(|field| {
                has_metadata || matches!(field.id.as_str(), "title" | "artist" | "album")
            })
            .flat_map(|field| {
                let Some((_, label, column)) = FIELDS.iter().find(|(id, _, _)| *id == field.id)
                else {
                    return Vec::new();
                };
                if *column == "genres" {
                    return crate::genres::links(item)
                        .into_iter()
                        .take(2)
                        .map(|(_, name)| (*label, name))
                        .collect();
                }
                let value = crate::metadata_format::player_text(item, column, language);
                (!value.is_empty())
                    .then_some((*label, value))
                    .into_iter()
                    .collect()
            })
            .collect()
    }
    pub fn move_field(&mut self, index: usize, offset: isize) {
        if index < 3 || index >= self.fields.len() {
            return;
        }
        let destination = index
            .saturating_add_signed(offset)
            .clamp(3, self.fields.len() - 1);
        self.fields.swap(index, destination);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_order_visibility_and_native_settings_round_trip() {
        let mut config = Config::default();
        assert!(config.active_tab.is_empty());
        for tab in ["", "lyrics", "queue", "related", "visualizer"] {
            config.active_tab = tab.into();
            config.validate().unwrap();
        }
        config
            .fields
            .iter_mut()
            .find(|field| field.id == "sample_rate")
            .unwrap()
            .disabled = false;
        config
            .fields
            .iter_mut()
            .find(|field| field.id == "title")
            .unwrap()
            .disabled = true;
        config.move_field(13, -9);
        let item = Item {
            name: "Song".into(),
            column_data: [("sampleRate".into(), "48000".into())].into(),
            ..Default::default()
        };
        assert!(
            !config
                .metadata(&item, "en")
                .iter()
                .any(|(label, _)| *label == "Title")
        );
        assert!(
            config
                .metadata(&item, "en")
                .iter()
                .any(|(label, text)| *label == "Sample rate" && text == "48 kHz")
        );
        config.validate().unwrap();
        assert_eq!(
            config,
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap()
        );
        config.opacity = f32::NAN;
        assert!(config.validate().is_err());
        config.opacity = 25.;
        config.fields.push(config.fields[0].clone());
        assert!(config.validate().is_err());
    }
}
