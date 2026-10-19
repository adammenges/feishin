//! Saved options for the original Play random form.
use crate::auto_dj::Mode;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Played {
    #[default]
    All,
    Never,
    Played,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(alias = "playbackKind")]
    pub kind: Mode,
    pub limit: usize,
    pub genre: String,
    pub music_folder: String,
    #[serde(alias = "enableMinYear")]
    pub enable_min_year: bool,
    #[serde(alias = "enableMaxYear")]
    pub enable_max_year: bool,
    #[serde(alias = "minYear")]
    pub min_year: u32,
    #[serde(alias = "maxYear")]
    pub max_year: u32,
    pub played: Played,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            kind: Mode::Songs,
            limit: 100,
            genre: String::new(),
            music_folder: String::new(),
            enable_min_year: false,
            enable_max_year: false,
            min_year: 2000,
            max_year: 2020,
            played: Played::All,
        }
    }
}
impl Config {
    pub fn min(&self) -> Option<u32> {
        (self.enable_min_year && self.min_year != 0).then_some(self.min_year)
    }
    pub fn max(&self) -> Option<u32> {
        (self.kind == Mode::Songs && self.enable_max_year && self.max_year != 0)
            .then_some(self.max_year)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=500).contains(&self.limit),
            "Choose 1 to 500 random picks"
        );
        ensure!(
            [self.min_year, self.max_year]
                .into_iter()
                .all(|year| year == 0 || (1850..=2050).contains(&year)),
            "Choose years between 1850 and 2050"
        );
        ensure!(
            self.min()
                .zip(self.max())
                .is_none_or(|(min, max)| min <= max),
            "From year must not exceed to year"
        );
        ensure!(
            self.genre.len() <= 512 && !self.genre.contains(['\0', '\r', '\n']),
            "Invalid random-play genre"
        );
        ensure!(
            self.music_folder.len() <= 256
                && !self
                    .music_folder
                    .contains(['/', '?', '#', '\0', '\r', '\n']),
            "Invalid random-play library ID"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_defaults_aliases_and_enabled_year_bounds() {
        let mut config = Config::default();
        assert_eq!(
            (config.limit, config.min_year, config.max_year),
            (100, 2000, 2020)
        );
        assert!(config.validate().is_ok());
        assert_eq!((config.min(), config.max()), (None, None));
        config.enable_min_year = true;
        config.enable_max_year = true;
        config.min_year = 2021;
        assert!(config.validate().is_err());
        config.kind = Mode::Albums;
        assert!(config.validate().is_ok());
        assert_eq!(config.max(), None);
        for limit in [0, 501, usize::MAX] {
            config.limit = limit;
            assert!(config.validate().is_err());
        }
        let imported: Config = serde_json::from_value(serde_json::json!({"playbackKind":"albums","limit":12,"enableMinYear":true,"minYear":1991,"enableMaxYear":true,"maxYear":1980,"played":"never"})).unwrap();
        assert_eq!(imported.kind, Mode::Albums);
        assert_eq!(imported.min(), Some(1991));
        assert_eq!(imported.max(), None);
        assert_eq!(imported.played, Played::Never);
        imported.validate().unwrap();
        assert_eq!(
            serde_json::from_value::<Config>(serde_json::to_value(&imported).unwrap()).unwrap(),
            imported
        );
    }
}
