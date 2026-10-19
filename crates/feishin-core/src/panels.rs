use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Panel {
    Queue,
    Lyrics,
    Visualizer,
}
impl Panel {
    pub const ALL: [Self; 3] = [Self::Queue, Self::Lyrics, Self::Visualizer];
    pub fn label(self) -> &'static str {
        match self {
            Self::Queue => "Queue",
            Self::Lyrics => "Lyrics",
            Self::Visualizer => "Visualizer",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub kind: String,
    pub layout: String,
    pub expanded: bool,
    pub width: f32,
    pub height: f32,
    pub queue: bool,
    pub lyrics: bool,
    pub visualizer: bool,
    pub combined: bool,
    pub order: Vec<Panel>,
    pub sizes: Vec<f32>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            kind: "sideQueue".into(),
            layout: "horizontal".into(),
            expanded: false,
            width: 600.,
            height: 320.,
            queue: true,
            lyrics: true,
            visualizer: true,
            combined: false,
            order: Panel::ALL.to_vec(),
            sizes: vec![],
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.kind.as_str(), "sideQueue" | "sideDrawerQueue"),
            "Invalid playback panel type"
        );
        ensure!(
            matches!(self.layout.as_str(), "horizontal" | "vertical"),
            "Invalid playback panel layout"
        );
        ensure!(
            self.width.is_finite()
                && (240. ..=1600.).contains(&self.width)
                && self.height.is_finite()
                && (160. ..=1200.).contains(&self.height),
            "Invalid playback panel dimensions"
        );
        ensure!(
            self.order.len() <= 3
                && self.order.iter().collect::<HashSet<_>>().len() == self.order.len(),
            "Duplicate playback panel"
        );
        ensure!(
            self.sizes.len() <= 3
                && self
                    .sizes
                    .iter()
                    .all(|size| size.is_finite() && (40. ..=1600.).contains(size)),
            "Invalid playback panel sizes"
        );
        Ok(())
    }
    pub fn enabled(
        &self,
        queue_visible: bool,
        lyrics_visible: bool,
        visualizer_visible: bool,
    ) -> Vec<Panel> {
        self.order
            .iter()
            .copied()
            .filter(|panel| match panel {
                Panel::Queue => self.queue && queue_visible,
                Panel::Lyrics => {
                    (self.lyrics && lyrics_visible)
                        || (self.combined && self.visualizer && visualizer_visible)
                }
                Panel::Visualizer => !self.combined && self.visualizer && visualizer_visible,
            })
            .collect()
    }
    pub fn move_panel(&mut self, index: usize, up: bool) {
        let target = if up {
            index.checked_sub(1)
        } else {
            index.checked_add(1)
        };
        if let Some(target) = target
            && index < self.order.len()
            && target < self.order.len()
        {
            self.order.swap(index, target);
        }
    }
    pub fn panel_size(&self, panel: Panel, index: usize, active: &[Panel]) -> Option<f32> {
        if panel == Panel::Queue || active.len() == 1 {
            return None;
        }
        let has_queue = active.contains(&Panel::Queue);
        if !has_queue && index + 1 == active.len() {
            return None;
        }
        if let Some(size) = self.sizes.get(index) {
            return Some(*size);
        }
        (active[..index].iter().all(|panel| *panel == Panel::Queue) || !has_queue).then_some(100.)
    }
    pub fn close_panel(&mut self, panel: Panel) {
        match panel {
            Panel::Lyrics => {
                self.lyrics = false;
                if self.combined {
                    self.visualizer = false;
                }
            }
            Panel::Visualizer => self.visualizer = false,
            Panel::Queue => self.queue = false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidebar_panels_match_original_visibility_sizes_and_hover_actions() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../assets/sidebar-panels-reference.json")).unwrap();
        for case in fixture["visibility"].as_array().unwrap() {
            let config = Config {
                order: serde_json::from_value(case["order"].clone()).unwrap(),
                queue: case["queue"].as_bool().unwrap(),
                lyrics: case["lyrics"].as_bool().unwrap(),
                visualizer: case["visualizer"].as_bool().unwrap(),
                combined: case["combined"].as_bool().unwrap(),
                ..Default::default()
            };
            assert_eq!(
                serde_json::to_value(config.enabled(
                    case["nowPlaying"] != true,
                    true,
                    case["webAudio"] == true
                ))
                .unwrap(),
                case["expected"],
                "{case}"
            );
        }
        for case in fixture["layout"].as_array().unwrap() {
            let config = Config {
                order: serde_json::from_value(case["order"].clone()).unwrap(),
                sizes: if case["saved"].is_null() {
                    vec![]
                } else {
                    serde_json::from_value(case["saved"].clone()).unwrap()
                },
                ..Default::default()
            };
            let sizes = config
                .order
                .iter()
                .enumerate()
                .map(|(index, panel)| config.panel_size(*panel, index, &config.order))
                .collect::<Vec<_>>();
            let expected: Vec<Option<f32>> =
                serde_json::from_value(case["expected"].clone()).unwrap();
            assert_eq!(sizes, expected, "{case}");
        }
        for case in fixture["actions"].as_array().unwrap() {
            let mut config = Config {
                order: serde_json::from_value(case["order"].clone()).unwrap(),
                combined: case["combined"].as_bool().unwrap(),
                sizes: vec![80., 140., 230.],
                ..Default::default()
            };
            let panel: Panel = serde_json::from_value(case["panel"].clone()).unwrap();
            let index = config
                .order
                .iter()
                .position(|value| *value == panel)
                .unwrap();
            assert_eq!(index > 0, case["canMoveUp"] == true);
            assert_eq!(index + 1 < config.order.len(), case["canMoveDown"] == true);
            match case["action"].as_str().unwrap() {
                "up" => config.move_panel(index, true),
                "down" => config.move_panel(index, false),
                "close" => config.close_panel(panel),
                _ => unreachable!(),
            }
            assert_eq!(config.sizes, [80., 140., 230.]);
            let expected = &case["expected"]["general"];
            if let Some(order) = expected.get("sidebarPanelOrder") {
                assert_eq!(serde_json::to_value(&config.order).unwrap(), *order);
            }
            if let Some(lyrics) = expected.get("showLyricsInSidebar") {
                assert_eq!(config.lyrics, *lyrics == true);
            }
            if let Some(visualizer) = expected.get("showVisualizerInSidebar") {
                assert_eq!(config.visualizer, *visualizer == true);
            }
        }
    }
    #[test]
    fn layouts_order_visibility_and_combined_panels_round_trip() {
        let mut config = Config::default();
        config.move_panel(2, true);
        assert_eq!(
            config.enabled(false, true, true),
            [Panel::Visualizer, Panel::Lyrics]
        );
        config.combined = true;
        config.lyrics = false;
        assert_eq!(
            config.enabled(true, false, true),
            [Panel::Queue, Panel::Lyrics]
        );
        let restored: Config =
            serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
        assert_eq!(restored, config);
        config.order.push(Panel::Queue);
        assert!(config.validate().is_err());
        config.order = Panel::ALL.to_vec();
        config.height = f32::NAN;
        assert!(config.validate().is_err());
    }
}
