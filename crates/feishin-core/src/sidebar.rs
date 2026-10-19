use crate::model::{Item, ItemKind};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const ITEMS: [&str; 17] = [
    "Now Playing",
    "Search",
    "Home",
    "Favorites",
    "Albums",
    "Tracks",
    "Artists",
    "Artists-all",
    "Genres",
    "Folders",
    "Playlists",
    "Collections",
    "Radio",
    "Settings",
    "Explore",
    "Queue",
    "Visualizer",
];
pub fn reorder(
    order: &mut Vec<String>,
    items: &[Item],
    moving: &[String],
    target: Option<&str>,
    after: bool,
) -> bool {
    let valid: HashSet<_> = items.iter().map(|item| item.id.as_str()).collect();
    let source = moving;
    let moving: HashSet<_> = moving
        .iter()
        .map(String::as_str)
        .filter(|id| valid.contains(id))
        .collect();
    if moving.is_empty() || target.is_some_and(|id| !valid.contains(id)) {
        return false;
    }
    let mut seen = HashSet::new();
    let current: Vec<_> = order
        .iter()
        .map(String::as_str)
        .chain(items.iter().map(|item| item.id.as_str()))
        .filter(|id| valid.contains(id) && seen.insert(*id))
        .map(str::to_owned)
        .collect();
    let mut seen = HashSet::new();
    let block: Vec<_> = source
        .iter()
        .filter(|id| moving.contains(id.as_str()) && seen.insert(id.as_str()))
        .cloned()
        .collect();
    let boundary = target.and_then(|target| current.iter().position(|id| id == target));
    let index = boundary.map(|boundary| {
        current[..boundary]
            .iter()
            .filter(|id| !moving.contains(id.as_str()))
            .count()
            + usize::from(after)
    });
    let mut remaining: Vec<_> = current
        .into_iter()
        .filter(|id| !moving.contains(id.as_str()))
        .collect();
    let index = index.unwrap_or(remaining.len()).min(remaining.len());
    remaining.splice(index..index, block);
    if *order == remaining {
        return false;
    }
    *order = remaining;
    true
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    #[serde(default)]
    pub disabled: bool,
}

/// Transient CSS width during a pointer resize. Persist only on release.
pub struct Resize {
    pub width: f32,
    collapsed_during_drag: bool,
}
impl Resize {
    pub fn new(config: &Config) -> Self {
        Self {
            width: config.width,
            collapsed_during_drag: false,
        }
    }
    pub fn update(&mut self, config: &mut Config, position: f32) {
        if !position.is_finite() {
            return;
        }
        if position < 160. {
            self.collapsed_during_drag = true;
            config.collapsed = true;
        } else {
            if self.collapsed_during_drag {
                self.collapsed_during_drag = false;
                config.collapsed = false;
            }
            self.width = position.clamp(260., 400.);
        }
    }
    pub fn finish(self, config: &mut Config) {
        if !self.collapsed_during_drag {
            config.collapsed = false;
            config.width = self.width;
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub items: Vec<Entry>,
    pub width: f32,
    pub collapsed: bool,
    pub collapsed_navigation: bool,
    pub playlist_list: bool,
    pub playlist_mode: String,
    pub playlist_filter_regex: String,
    pub playlist_sorting: bool,
    pub collapse_shared: bool,
    pub image_enabled: bool,
    pub image_shown: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            items: ITEMS
                .into_iter()
                .map(|id| Entry {
                    id: id.into(),
                    disabled: matches!(
                        id,
                        "Now Playing"
                            | "Search"
                            | "Playlists"
                            | "Settings"
                            | "Explore"
                            | "Queue"
                            | "Visualizer"
                    ),
                })
                .collect(),
            width: 400.,
            collapsed: false,
            collapsed_navigation: true,
            playlist_list: true,
            playlist_mode: "expanded".into(),
            playlist_filter_regex: String::new(),
            playlist_sorting: false,
            collapse_shared: false,
            image_enabled: true,
            image_shown: false,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        let mut seen = HashSet::new();
        ensure!(
            self.items.len() <= ITEMS.len()
                && self
                    .items
                    .iter()
                    .all(|item| ITEMS.contains(&item.id.as_str()) && seen.insert(&item.id)),
            "Invalid or duplicate sidebar item"
        );
        ensure!(
            self.width.is_finite() && (160.0..=640.0).contains(&self.width),
            "Sidebar width must be between 160 and 640"
        );
        ensure!(
            matches!(self.playlist_mode.as_str(), "compact" | "expanded"),
            "Invalid sidebar playlist mode"
        );
        ensure!(
            self.playlist_filter_regex.len() <= 4096,
            "Playlist filter is too long"
        );
        Ok(())
    }
    pub fn enabled(&self, id: &str) -> bool {
        self.items
            .iter()
            .any(|item| item.id == id && !item.disabled)
    }
    pub fn move_entry(&mut self, index: usize, up: bool) {
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
    pub fn playlists(&self, items: &[Item], order: &[String]) -> Vec<Item> {
        let regex = crate::playback_filters::regex(&self.playlist_filter_regex);
        let rank: HashMap<_, _> = order
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect();
        let mut items: Vec<_> = items
            .iter()
            .filter(|item| {
                item.kind == ItemKind::Playlist
                    && !regex.as_ref().is_some_and(|regex| {
                        regex
                            .find_from_ucs2(&item.name.encode_utf16().collect::<Vec<_>>(), 0)
                            .next()
                            .is_some()
                    })
            })
            .cloned()
            .collect();
        if self.playlist_sorting {
            items.sort_by_key(|item| rank.get(item.id.as_str()).copied().unwrap_or(usize::MAX));
        }
        items
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn dragged_playlist_row_edges_match_original_callback() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/drag-placement-reference.json")).unwrap();
        let ids = |value: &serde_json::Value| {
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        for case in reference["cases"].as_array().unwrap() {
            let mut order = ids(&case["current"]);
            let items = order
                .iter()
                .map(|id| crate::model::Item {
                    id: id.clone(),
                    kind: crate::model::ItemKind::Playlist,
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            super::reorder(
                &mut order,
                &items,
                &ids(&case["source"]),
                case["target"].as_str(),
                case["edge"] == "bottom",
            );
            assert_eq!(order, ids(&case["sidebar"]), "{case}");
        }
    }

    use super::*;
    #[test]
    fn original_navigation_defaults_and_drag_resize_sequences() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/sidebar-navigation-reference.json"))
                .unwrap();
        let config = Config::default();
        for (entry, original) in config
            .items
            .iter()
            .zip(reference["items"].as_array().unwrap())
        {
            assert_eq!(entry.id, original["id"].as_str().unwrap());
            assert_eq!(entry.disabled, original["disabled"].as_bool().unwrap());
        }
        assert!(config.items.iter().skip(14).all(|entry| entry.disabled));
        assert_eq!(
            format!("{}px", config.width),
            reference["defaults"]["leftWidth"].as_str().unwrap()
        );
        assert_eq!(
            config.collapsed,
            reference["defaults"]["collapsed"].as_bool().unwrap()
        );
        assert_eq!(
            config.image_shown,
            reference["defaults"]["image"].as_bool().unwrap()
        );
        for case in reference["cases"].as_array().unwrap() {
            let mut config = Config {
                width: case["width"].as_f64().unwrap() as f32,
                collapsed: case["collapsed"].as_bool().unwrap(),
                ..Default::default()
            };
            let mut resize = Resize::new(&config);
            for frame in case["frames"].as_array().unwrap() {
                resize.update(&mut config, frame["x"].as_f64().unwrap() as f32);
                assert_eq!(
                    config.collapsed,
                    frame["collapsed"].as_bool().unwrap(),
                    "{case}; {frame}"
                );
                assert_eq!(
                    if config.collapsed { 80. } else { resize.width },
                    frame["width"].as_f64().unwrap() as f32,
                    "{case}; {frame}"
                );
                assert_eq!(config.width, frame["savedWidth"].as_f64().unwrap() as f32);
            }
            resize.finish(&mut config);
            assert_eq!(
                config.width,
                case["final"]["width"].as_f64().unwrap() as f32,
                "{case}"
            );
            assert_eq!(
                config.collapsed,
                case["final"]["collapsed"].as_bool().unwrap(),
                "{case}"
            );
        }
    }
    #[test]
    fn original_javascript_playlist_filters_scopes_and_order() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../assets/sidebar-playlists-reference.json"))
                .unwrap();
        let items = fixture["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| Item {
                kind: ItemKind::Playlist,
                id: value["id"].as_str().unwrap().into(),
                name: value["name"].as_str().unwrap().into(),
                owner_name: value["owner"].as_str().unwrap().into(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let order = fixture["order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        for case in fixture["cases"].as_array().unwrap() {
            let config = Config {
                playlist_filter_regex: case["pattern"].as_str().unwrap().into(),
                playlist_sorting: case["sorted"].as_bool().unwrap(),
                ..Default::default()
            };
            config.validate().unwrap();
            let ids = config
                .playlists(&items, &order)
                .into_iter()
                .filter(|item| {
                    (item.owner_name.is_empty() || item.owner_name == "me")
                        == (case["scope"] == "owned")
                })
                .map(|item| item.id)
                .collect::<Vec<_>>();
            let expected = case["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(ids, expected, "Original sidebar case: {case}");
        }
    }
    #[test]
    fn sidebar_order_filters_and_collapsed_preferences_round_trip() {
        let mut config = Config {
            collapsed: true,
            playlist_sorting: true,
            playlist_filter_regex: "secret|hide".into(),
            ..Default::default()
        };
        config.move_entry(2, true);
        config.items[0].disabled = true;
        assert!(!config.enabled("Now Playing"));
        let items: Vec<_> = ["First", "SECRET", "Third", "Fourth"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| Item {
                id: index.to_string(),
                name: name.into(),
                kind: ItemKind::Playlist,
                ..Default::default()
            })
            .collect();
        assert_eq!(
            config
                .playlists(&items, &["2".into(), "removed".into(), "0".into()])
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["Third", "First", "Fourth"]
        );
        assert_eq!(
            serde_json::from_slice::<Config>(&serde_json::to_vec(&config).unwrap()).unwrap(),
            config
        );
        assert!(config.validate().is_ok());
        let mut order = vec!["removed".into(), "0".into(), "0".into()];
        assert!(reorder(
            &mut order,
            &items,
            &["2".into(), "1".into()],
            Some("0"),
            false
        ));
        assert_eq!(order, ["2", "1", "0", "3"]);
        assert!(!reorder(
            &mut order,
            &items,
            &["1".into()],
            Some("1"),
            false
        ));
        assert!(!reorder(
            &mut order,
            &items,
            &["missing".into()],
            None,
            false
        ));
        let mut nodes = crate::playlist_folders::tree(&items, "/", false);
        crate::playlist_folders::order(&mut nodes, &order);
        let ids: Vec<_> = nodes
            .iter()
            .filter_map(|node| {
                if let crate::playlist_folders::Node::Playlist { item, .. } = node {
                    Some(item.id.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(ids, ["2", "1", "0", "3"]);
        config.width = f32::NAN;
        assert!(config.validate().is_err());
        config.width = 210.;
        config.playlist_filter_regex = "[".into();
        assert!(config.validate().is_ok());
        assert_eq!(config.playlists(&items, &[]).len(), items.len());
    }
}
