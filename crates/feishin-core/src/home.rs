use crate::model::{ItemKind, ListQuery, ServerType, Sort};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub fn feature_columns(width: f32) -> usize {
    if width >= 2560. {
        6
    } else if width >= 1440. {
        5
    } else if width >= 1024. {
        4
    } else if width >= 768. {
        3
    } else {
        2
    }
}

pub fn feature_next(start: usize, count: usize, step: usize, forward: bool) -> usize {
    if count == 0 {
        return 0;
    }
    let start = start % count;
    if count <= step {
        return start;
    }
    if forward {
        (start + step) % count
    } else {
        (start + count - step) % count
    }
}

pub fn feature_title_size(title: &str, viewport: f32, rem: f32) -> f32 {
    let length: f32 = title.chars().map(|ch| if matches!(ch as u32, 0x4e00..=0x9fff | 0x3040..=0x30ff | 0x3400..=0x4dbf | 0xf900..=0xfaff | 0xff01..=0xff5e) { 2.5 } else { 1. }).sum();
    let percent = if length > 90. {
        1.3
    } else if length > 80. {
        1.4
    } else if length > 70. {
        1.5
    } else if length > 60. {
        1.75
    } else if length > 50. {
        1.875
    } else if length > 40. {
        2.
    } else if length > 30. {
        2.25
    } else if length > 20. {
        2.5
    } else {
        3.
    };
    (viewport * percent / 100.).clamp(rem * 1.75, rem * 2.75)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Genres,
    Random,
    RecentlyAdded,
    RecentlyReleased,
    RecentlyPlayed,
    MostPlayed,
    Playlists,
    Feature,
}
impl Kind {
    pub const SECTIONS: [Self; 7] = [
        Self::Genres,
        Self::Random,
        Self::RecentlyAdded,
        Self::RecentlyReleased,
        Self::RecentlyPlayed,
        Self::MostPlayed,
        Self::Playlists,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Genres => "Featured genres",
            Self::Random => "Explore",
            Self::RecentlyAdded => "Recently added",
            Self::RecentlyReleased => "Recently released",
            Self::RecentlyPlayed => "Recently played",
            Self::MostPlayed => "Most played",
            Self::Playlists => "Playlists",
            Self::Feature => "Featured albums",
        }
    }
    pub fn title_key(self) -> &'static str {
        match self {
            Self::Genres => "page.home.genres",
            Self::Random => "page.home.explore",
            Self::RecentlyAdded => "page.home.newlyAdded",
            Self::RecentlyReleased => "page.home.recentlyReleased",
            Self::RecentlyPlayed => "page.home.recentlyPlayed",
            Self::MostPlayed => "page.home.mostPlayed",
            Self::Playlists => "page.home.playlists",
            Self::Feature => "setting.homeFeature",
        }
    }
    pub fn query(self, server: ServerType, offset: usize) -> ListQuery {
        let (kind, sort, descending) = match self {
            Self::Genres => (ItemKind::Genre, Sort::Name, false),
            Self::Random | Self::Feature => (ItemKind::Album, Sort::Random, false),
            Self::RecentlyAdded => (ItemKind::Album, Sort::Newest, true),
            Self::RecentlyReleased => (ItemKind::Album, Sort::ReleaseDate, true),
            Self::RecentlyPlayed | Self::MostPlayed => (
                if server == ServerType::Jellyfin {
                    ItemKind::Song
                } else {
                    ItemKind::Album
                },
                if self == Self::RecentlyPlayed {
                    Sort::Recent
                } else {
                    Sort::Frequent
                },
                true,
            ),
            Self::Playlists => (ItemKind::Playlist, Sort::UpdatedAt, true),
        };
        ListQuery {
            kind,
            sort,
            descending,
            offset,
            limit: 20,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Section {
    pub id: Kind,
    #[serde(default)]
    pub disabled: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub feature: bool,
    pub feature_style: String,
    pub items: Vec<Section>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            feature: true,
            feature_style: "single".into(),
            items: Kind::SECTIONS
                .into_iter()
                .map(|id| Section {
                    id,
                    disabled: false,
                })
                .collect(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.feature_style.as_str(), "single" | "multiple"),
            "Invalid home feature style"
        );
        let mut seen = HashSet::new();
        ensure!(
            self.items.len() <= Kind::SECTIONS.len()
                && self
                    .items
                    .iter()
                    .all(|item| item.id != Kind::Feature && seen.insert(item.id)),
            "Invalid or duplicate home section"
        );
        Ok(())
    }
    pub fn enabled(&self) -> impl Iterator<Item = Kind> + '_ {
        self.feature.then_some(Kind::Feature).into_iter().chain(
            self.items
                .iter()
                .filter(|item| !item.disabled)
                .map(|item| item.id),
        )
    }
    pub fn move_section(&mut self, index: usize, up: bool) {
        let target = if up {
            index.checked_sub(1)
        } else {
            index.checked_add(1)
        };
        if let Some(target) = target
            && index < self.items.len()
            && target < self.items.len()
        {
            self.items.swap(index, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feature_carousel_width_and_wraparound_match_original_callbacks() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/home-feature-carousel-reference.json"
        ))
        .unwrap();
        for row in reference["rows"].as_array().unwrap() {
            assert_eq!(
                feature_columns(row["width"].as_f64().unwrap() as f32),
                row["columns"].as_u64().unwrap() as usize
            );
        }
        for case in reference["navigation"].as_array().unwrap() {
            let step = if case["style"] == "single" {
                1
            } else {
                feature_columns(case["width"].as_f64().unwrap() as f32)
            };
            assert_eq!(
                feature_next(
                    case["start"].as_u64().unwrap() as usize,
                    case["count"].as_u64().unwrap() as usize,
                    step,
                    case["forward"].as_bool().unwrap()
                ),
                case["next"].as_u64().unwrap() as usize,
                "{case}"
            );
        }
        assert_eq!(feature_next(0, 0, 2, true), 0);
        for case in reference["titles"].as_array().unwrap() {
            let actual = feature_title_size(
                case["title"].as_str().unwrap(),
                case["viewport"].as_f64().unwrap() as f32,
                case["rem"].as_f64().unwrap() as f32,
            );
            assert!(
                (f64::from(actual) - case["size"].as_f64().unwrap()).abs() < 0.00001,
                "{case}: {actual}"
            );
        }
    }
    #[test]
    fn home_sections_preserve_order_visibility_and_backend_query_semantics() {
        let mut config = Config::default();
        config.items[0].disabled = true;
        config.move_section(6, true);
        assert_eq!(
            config.enabled().collect::<Vec<_>>(),
            [
                Kind::Feature,
                Kind::Random,
                Kind::RecentlyAdded,
                Kind::RecentlyReleased,
                Kind::RecentlyPlayed,
                Kind::Playlists,
                Kind::MostPlayed
            ]
        );
        assert_eq!(
            serde_json::from_slice::<Config>(&serde_json::to_vec(&config).unwrap()).unwrap(),
            config
        );
        assert!(config.validate().is_ok());
        for server in [
            ServerType::Navidrome,
            ServerType::Subsonic,
            ServerType::Jellyfin,
        ] {
            let recent = Kind::RecentlyPlayed.query(server, 40);
            assert_eq!(
                (recent.offset, recent.limit, recent.sort, recent.descending),
                (40, 20, Sort::Recent, true)
            );
            assert_eq!(
                recent.kind,
                if server == ServerType::Jellyfin {
                    ItemKind::Song
                } else {
                    ItemKind::Album
                }
            );
            assert_eq!(
                Kind::RecentlyReleased.query(server, 0).sort,
                Sort::ReleaseDate
            );
            assert_eq!(Kind::MostPlayed.query(server, 0).sort, Sort::Frequent);
            let playlists = Kind::Playlists.query(server, 20);
            assert_eq!(
                (
                    playlists.kind,
                    playlists.sort,
                    playlists.descending,
                    playlists.offset,
                    playlists.limit
                ),
                (ItemKind::Playlist, Sort::UpdatedAt, true, 20, 20)
            );
        }
        config.items.push(config.items[0].clone());
        assert!(config.validate().is_err());
    }
}
