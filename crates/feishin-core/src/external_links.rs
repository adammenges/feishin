use crate::model::{Item, ItemKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub last_fm: bool,
    pub listen_brainz: bool,
    pub music_brainz: bool,
    pub qobuz: bool,
    pub spotify: bool,
    pub native_spotify: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            last_fm: true,
            listen_brainz: true,
            music_brainz: true,
            qobuz: true,
            spotify: true,
            native_spotify: false,
        }
    }
}
fn encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}
fn mbid(value: &str) -> Option<String> {
    uuid::Uuid::parse_str(value).ok().map(|id| id.to_string())
}
impl Config {
    pub fn links(&self, item: &Item) -> Vec<(&'static str, String)> {
        let album = item.kind == ItemKind::Album;
        if !self.enabled
            || !matches!(
                item.kind,
                ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
            )
        {
            return vec![];
        }
        let artist = if album {
            if item.album_artist.is_empty() {
                &item.artist
            } else {
                &item.album_artist
            }
        } else {
            &item.name
        };
        let query = if album {
            [artist.as_str(), &item.name]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            artist.clone()
        };
        let query = encode(query.trim());
        let mut links = vec![];
        if self.last_fm {
            links.push((
                "Last.fm",
                if album {
                    format!(
                        "https://www.last.fm/music/{}/{}",
                        encode(artist),
                        encode(&item.name)
                    )
                } else {
                    format!("https://www.last.fm/music/{}", encode(artist))
                },
            ));
        }
        if self.music_brainz
            && let Some(id) = mbid(&item.musicbrainz_id)
        {
            links.push((
                "MusicBrainz",
                format!(
                    "https://musicbrainz.org/{}/{id}",
                    if album { "release" } else { "artist" }
                ),
            ));
        }
        if self.listen_brainz {
            let id = mbid(if album {
                &item.musicbrainz_release_group_id
            } else {
                &item.musicbrainz_id
            });
            if let Some(id) = id {
                links.push((
                    "ListenBrainz",
                    format!(
                        "https://listenbrainz.org/{}/{id}",
                        if album { "album" } else { "artist" }
                    ),
                ));
            } else if !query.is_empty() {
                links.push((
                    "ListenBrainz",
                    format!("https://listenbrainz.org/search/?search_term={query}"),
                ));
            }
        }
        if self.qobuz && !query.is_empty() {
            links.push((
                "Qobuz",
                format!(
                    "https://www.qobuz.com/us-en/search/{}/{query}",
                    if album { "albums" } else { "artists" }
                ),
            ));
        }
        if self.spotify {
            links.push((
                "Spotify",
                if self.native_spotify {
                    format!("spotify:search:{}", query.replace("%20", "+"))
                } else {
                    format!("https://open.spotify.com/search/{query}")
                },
            ));
        }
        links
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn album_artist_provider_links_escape_names_and_respect_preferences() {
        let id = "123e4567-e89b-12d3-a456-426614174000";
        let mut item = Item {
            kind: ItemKind::Album,
            name: "Hello / 雨 & #".into(),
            album_artist: "A+B".into(),
            musicbrainz_id: id.into(),
            musicbrainz_release_group_id: id.into(),
            ..Default::default()
        };
        let mut config = Config::default();
        let links = config.links(&item);
        assert_eq!(links.len(), 5);
        assert!(
            links[0]
                .1
                .contains("A%2BB/Hello%20%2F%20%E9%9B%A8%20%26%20%23")
        );
        assert_eq!(links[1].1, format!("https://musicbrainz.org/release/{id}"));
        assert_eq!(links[2].1, format!("https://listenbrainz.org/album/{id}"));
        assert!(links.iter().all(|(_, link)| url::Url::parse(link).is_ok()));
        config.native_spotify = true;
        assert!(
            config
                .links(&item)
                .last()
                .unwrap()
                .1
                .starts_with("spotify:search:")
        );
        assert_eq!(
            config.links(&item).last().unwrap().1,
            "spotify:search:A%2BB+Hello+%2F+%E9%9B%A8+%26+%23"
        );
        item.name = "A Tribe Called Quest".into();
        item.kind = ItemKind::AlbumArtist;
        assert_eq!(
            config.links(&item).last().unwrap().1,
            "spotify:search:A+Tribe+Called+Quest"
        );
        item.musicbrainz_id = "../../bad?".into();
        let links = config.links(&item);
        assert_eq!(links.len(), 4);
        assert!(
            links[1]
                .1
                .starts_with("https://listenbrainz.org/search/?search_term=")
        );
        config.last_fm = false;
        config.spotify = false;
        assert_eq!(config.links(&item).len(), 2);
        config.enabled = false;
        assert!(config.links(&item).is_empty());
        config.enabled = true;
        item.kind = ItemKind::Song;
        assert!(config.links(&item).is_empty());
    }
}
