use crate::{
    api::Api,
    model::{Item, ItemKind, ListQuery, ServerType, Sort},
};
use anyhow::{Result, ensure};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Songs,
    Albums,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    #[default]
    Similar,
    LibraryRandom,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub mode: Mode,
    pub song_strategy: Strategy,
    pub album_strategy: Strategy,
    pub item_count: usize,
    pub timing: usize,
    pub allow_duplicates: bool,
    pub only_similar: bool,
    pub music_folder: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Songs,
            song_strategy: Strategy::Similar,
            album_strategy: Strategy::Similar,
            item_count: 5,
            timing: 1,
            allow_duplicates: false,
            only_similar: false,
            music_folder: None,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=50).contains(&self.item_count) && (1..=5).contains(&self.timing),
            "Auto DJ needs 1 to 50 items and a timing threshold of 1 to 5 tracks"
        );
        ensure!(
            self.music_folder
                .as_ref()
                .is_none_or(|value| value.len() <= 256 && !value.contains(['/', '?', '#', '\0'])),
            "Invalid Auto DJ library ID"
        );
        Ok(())
    }
}
fn select(
    mut pool: Vec<Item>,
    selected: &mut Vec<Item>,
    excluded: &mut HashSet<String>,
    config: &Config,
) {
    pool.shuffle(&mut rand::thread_rng());
    for item in pool {
        if selected.len() >= config.item_count {
            break;
        }
        if !item.id.is_empty() && (config.allow_duplicates || excluded.insert(item.id.clone())) {
            selected.push(item);
        }
    }
}
fn as_albums(songs: Vec<Item>) -> Vec<Item> {
    songs
        .into_iter()
        .filter(|item| !item.album_id.is_empty())
        .map(|item| Item {
            id: item.album_id,
            name: item.album,
            server_id: item.server_id,
            kind: ItemKind::Album,
            ..Default::default()
        })
        .collect()
}
impl Api {
    pub fn auto_dj(&self, current: &Item, config: &Config, queue: &[Item]) -> Result<Vec<Item>> {
        let selected = self.auto_dj_selection(current, config, queue)?;
        if config.mode == Mode::Albums {
            let mut songs = vec![];
            for album in selected {
                songs.extend(self.all_tracks(&album)?);
            }
            Ok(songs)
        } else {
            Ok(selected)
        }
    }

    pub(crate) fn auto_dj_selection(
        &self,
        current: &Item,
        config: &Config,
        queue: &[Item],
    ) -> Result<Vec<Item>> {
        config.validate()?;
        ensure!(
            current.server_id == self.server.id
                && current.kind == ItemKind::Song
                && !current.id.is_empty(),
            "Auto DJ needs a music track on this server"
        );
        let albums = config.mode == Mode::Albums;
        let strategy = if albums {
            config.album_strategy
        } else {
            config.song_strategy
        };
        let folder = config.music_folder.as_deref().or_else(|| {
            self.server
                .options
                .music_folder_ids
                .first()
                .map(String::as_str)
        });
        // The original server feature catalogs do not enable similarSongsMusicFolder.
        let try_similar = folder.is_none();
        let mut excluded = queue
            .iter()
            .filter(|item| item.server_id == current.server_id)
            .map(|item| {
                if albums {
                    item.album_id.clone()
                } else {
                    item.id.clone()
                }
            })
            .collect::<HashSet<_>>();
        let mut selected = vec![];
        let pool = |limit, genre: Option<&str>, artist: Option<&str>| -> Result<Vec<Item>> {
            if !albums && artist.is_none() {
                return self.random_songs_filtered(limit, None, genre, folder);
            }
            let mut query = ListQuery {
                kind: if albums {
                    ItemKind::Album
                } else {
                    ItemKind::Song
                },
                limit,
                sort: Sort::Random,
                folder_id: config.music_folder.clone(),
                ..Default::default()
            };
            if self.server.kind == ServerType::Jellyfin {
                query.filters.genre_ids = genre.into_iter().map(str::to_owned).collect();
            } else {
                query.genre = genre.map(str::to_owned);
            }
            if self.server.kind == ServerType::Navidrome {
                // The original song controller maps albumArtistIds through its versioned artist key.
                query.filters.artist_ids = artist.into_iter().map(str::to_owned).collect();
            } else {
                query.artist_id = artist.map(str::to_owned);
            }
            Ok(self.list(&query)?.items)
        };
        if strategy == Strategy::LibraryRandom {
            select(
                pool(
                    if albums {
                        config.item_count
                    } else {
                        (config.item_count * 3).max(50)
                    },
                    None,
                    None,
                )?,
                &mut selected,
                &mut excluded,
                config,
            );
            return Ok(selected);
        }
        if try_similar {
            let songs = self.random_songs_filtered(
                if albums {
                    config.item_count * 4
                } else {
                    config.item_count
                },
                Some(current),
                None,
                None,
            )?;
            select(
                if albums { as_albums(songs) } else { songs },
                &mut selected,
                &mut excluded,
                config,
            );
            if config.only_similar && !selected.is_empty() {
                return Ok(selected);
            }
        }
        if selected.len() < config.item_count
            && let Some((id, name)) = crate::genres::links(current).first()
        {
            let genre = if (albums && self.server.kind != ServerType::Jellyfin)
                || (id.is_empty() && current.genre_links.is_none())
            {
                // Older native queues retained only the display name.
                name
            } else {
                id
            };
            let genre_pool = pool(50, Some(genre), None)?;
            if !try_similar {
                let mix_config = Config {
                    item_count: config.item_count.min(selected.len() + 10),
                    ..config.clone()
                };
                select(
                    pool(10, None, None)?,
                    &mut selected,
                    &mut excluded,
                    &mix_config,
                );
            }
            select(genre_pool, &mut selected, &mut excluded, config);
            if config.only_similar && !selected.is_empty() {
                return Ok(selected);
            }
        }
        if selected.len() < config.item_count
            && let Some((artist, _)) = crate::artists::links(current, true).first()
        {
            select(
                pool(50, None, Some(artist))?,
                &mut selected,
                &mut excluded,
                config,
            );
            if config.only_similar && !selected.is_empty() {
                return Ok(selected);
            }
        }
        if selected.len() < config.item_count {
            select(
                pool(if albums { 80 } else { 50 }, None, None)?,
                &mut selected,
                &mut excluded,
                config,
            );
        }
        Ok(selected)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_respects_duplicate_policy_item_limit_and_album_identity() {
        let pool = ["queued", "new", "new", "other"]
            .map(|id| Item {
                id: id.into(),
                album_id: "album".into(),
                ..Default::default()
            })
            .to_vec();
        let mut selected = vec![];
        let mut excluded = HashSet::from(["queued".into()]);
        select(
            pool.clone(),
            &mut selected,
            &mut excluded,
            &Config {
                item_count: 3,
                ..Default::default()
            },
        );
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().all(|item| item.id != "queued"));
        let mut selected = vec![];
        select(
            pool.clone(),
            &mut selected,
            &mut excluded,
            &Config {
                allow_duplicates: true,
                item_count: 4,
                ..Default::default()
            },
        );
        assert_eq!(selected.len(), 4);
        let mut selected = vec![];
        let mut excluded = HashSet::new();
        select(
            as_albums(pool),
            &mut selected,
            &mut excluded,
            &Config::default(),
        );
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].kind, ItemKind::Album);
        assert!(
            Config {
                timing: 0,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
