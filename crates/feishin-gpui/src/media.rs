use crate::app::Feishin;
use feishin_core::{
    images::Placement,
    model::{Item, ItemKind},
};
use souvlaki::{MediaMetadata, MediaPlayback, MediaPosition, MprisMetadata};
use std::{path::Path, time::Duration};

#[derive(Default, PartialEq)]
pub(crate) struct Metadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    cover_url: Option<String>,
    duration: Option<Duration>,
    mpris: MprisMetadata,
}

impl Metadata {
    fn for_item(
        item: &Item,
        key: uuid::Uuid,
        cover: Option<&Path>,
        lyrics: Option<String>,
    ) -> Self {
        let nonempty = |value: &String| (!value.is_empty()).then(|| value.clone());
        let names = |values: &[String], fallback: &String| {
            if values.is_empty() {
                nonempty(fallback).into_iter().collect()
            } else {
                values.to_vec()
            }
        };
        let nonzero = |value: u32| i32::try_from(value).ok().filter(|value| *value > 0);
        let mut metadata = Self {
            title: nonempty(&item.name),
            artist: nonempty(&item.artist),
            album: nonempty(&item.album),
            cover_url: cover
                .and_then(|path| url::Url::from_file_path(path).ok())
                .map(Into::into),
            duration: Duration::try_from_secs_f64(item.duration)
                .ok()
                .filter(|value| !value.is_zero()),
            mpris: MprisMetadata {
                // Queue identities distinguish duplicates and always form valid object paths.
                track_id: format!("/org/mpris/MediaPlayer2/track/{}", key.simple()),
                artists: names(&item.artist_names, &item.artist),
                album_artists: names(&item.album_artist_names, &item.album_artist),
                genres: names(&item.genre_names, &item.genre),
                lyrics,
                comment: nonempty(&item.comment),
                content_created: item.column_data.get("releaseDate").and_then(nonempty),
                last_used: item.column_data.get("lastPlayedAt").and_then(nonempty),
                audio_bpm: item
                    .column_data
                    .get("bpm")
                    .and_then(|value| value.parse().ok()),
                disc_number: nonzero(item.disc),
                track_number: nonzero(item.track),
                use_count: Some(item.play_count.min(i32::MAX as u32) as i32),
                user_rating: (item.rating > 0).then(|| f64::from(item.rating.min(5)) / 5.),
            },
        };
        if item.kind == ItemKind::Radio {
            metadata.cover_url = None;
            metadata.duration = None;
            metadata.mpris = MprisMetadata {
                track_id: metadata.mpris.track_id,
                artists: metadata.mpris.artists,
                ..Default::default()
            };
        }
        metadata
    }

    fn borrowed(&self) -> MediaMetadata<'_> {
        MediaMetadata {
            title: self.title.as_deref(),
            artist: self.artist.as_deref(),
            album: self.album.as_deref(),
            cover_url: self.cover_url.as_deref(),
            duration: self.duration,
            mpris: Some(&self.mpris),
        }
    }
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn loop_status(repeat: feishin_core::queue::Repeat) -> souvlaki::LoopStatus {
    use feishin_core::queue::Repeat;
    use souvlaki::LoopStatus;
    match repeat {
        Repeat::Off => LoopStatus::None,
        Repeat::One => LoopStatus::Track,
        Repeat::All => LoopStatus::Playlist,
    }
}

impl Feishin {
    pub(crate) fn update_media(&mut self) {
        if self.media.is_none() {
            self.media_metadata = None;
            return;
        }
        let metadata = self
            .current_metadata()
            .zip(self.current_entry())
            .map(|(item, entry)| {
                let lyrics = (!self.lyrics.is_empty())
                    .then(|| {
                        feishin_core::lyric_export::render(
                            &self.lyric_export_metadata(),
                            self.lyric_offset() * 1000.,
                            self.lyrics.iter().any(|line| line.time.is_some()),
                        )
                        .ok()
                    })
                    .flatten();
                Metadata::for_item(
                    &item,
                    entry.key,
                    self.artwork_path_at(&item, Placement::ItemCard)
                        .map(|path| path.as_path()),
                    lyrics,
                )
            })
            .unwrap_or_default();
        let progress = Duration::try_from_secs_f64(self.current_position().max(0.))
            .ok()
            .map(MediaPosition);
        let media = self.media.as_mut().unwrap();
        if self.media_metadata.as_ref() != Some(&metadata)
            && media.set_metadata(metadata.borrowed()).is_ok()
        {
            self.media_metadata = Some(metadata);
        }
        let _ = media.set_playback(if self.playing {
            MediaPlayback::Playing { progress }
        } else if self.loaded {
            MediaPlayback::Paused { progress }
        } else {
            MediaPlayback::Stopped
        });
        #[cfg(target_os = "linux")]
        {
            let _ = media.set_volume(f64::from(self.state.settings.volume) / 100.);
            let _ = media.set_shuffle(self.state.queue.shuffle);
            let _ = media.set_loop_status(loop_status(self.state.queue.repeat));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn desktop_metadata_matches_original_linux_bridge() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/mpris-reference.json")).unwrap();
        for case in reference["metadata"].as_array().unwrap() {
            let source = &case["song"];
            let names = |field: &str| {
                source[field]
                    .as_array()
                    .map(|values| {
                        values
                            .iter()
                            .map(|value| value["name"].as_str().unwrap().to_owned())
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let string = |field: &str| source[field].as_str().unwrap_or_default().to_owned();
            let item = Item {
                id: string("id"),
                name: string("name"),
                album: string("album"),
                artist_names: names("artists"),
                album_artist_names: names("albumArtists"),
                genre_names: names("genres"),
                duration: source["duration"].as_f64().unwrap_or_default() / 1000.,
                comment: string("comment"),
                track: source["trackNumber"].as_u64().unwrap_or_default() as u32,
                disc: source["discNumber"].as_u64().unwrap_or_default() as u32,
                rating: source["userRating"].as_u64().unwrap_or_default() as u8,
                play_count: source["playCount"].as_u64().unwrap_or_default() as u32,
                column_data: ["bpm", "releaseDate", "lastPlayedAt"]
                    .into_iter()
                    .filter(|key| !source[key].is_null())
                    .map(|key| {
                        (
                            key.into(),
                            source[key]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| source[key].to_string()),
                        )
                    })
                    .collect(),
                kind: if source["_serverId"] == "" {
                    ItemKind::Radio
                } else {
                    ItemKind::Song
                },
                ..Default::default()
            };
            let key = uuid::Uuid::new_v4();
            let metadata = if source.is_null() {
                Metadata::default()
            } else {
                Metadata::for_item(
                    &item,
                    key,
                    case["cover"].as_str().map(|_| {
                        Path::new(if cfg!(windows) {
                            "C:/tmp/art work.png"
                        } else {
                            "/tmp/art work.png"
                        })
                    }),
                    None,
                )
            };
            let extra = &metadata.mpris;
            let mut actual = json!({
                "mpris:artUrl": metadata.cover_url, "mpris:length": metadata.duration.map(|value| value.as_micros() as u64),
                "xesam:title": metadata.title, "xesam:album": metadata.album,
                "xesam:artist": extra.artists, "xesam:albumArtist": extra.album_artists, "xesam:genre": extra.genres,
                "xesam:audioBpm": extra.audio_bpm, "xesam:comment": extra.comment.as_ref().map(|value| vec![value]),
                "xesam:contentCreated": extra.content_created, "xesam:lastUsed": extra.last_used,
                "xesam:discNumber": extra.disc_number, "xesam:trackNumber": extra.track_number,
                "xesam:useCount": extra.use_count, "xesam:userRating": extra.user_rating,
            });
            let mut expected = case["metadata"].clone();
            expected.as_object_mut().unwrap().remove("mpris:trackid");
            if cfg!(windows) && !expected["mpris:artUrl"].is_null() {
                expected["mpris:artUrl"] = json!("file:///C:/tmp/art%20work.png");
            }
            for value in [&mut actual, &mut expected] {
                value.as_object_mut().unwrap().retain(|_, value| {
                    !value.is_null() && value.as_array().is_none_or(|values| !values.is_empty())
                });
            }
            assert_eq!(actual, expected, "{}", case["name"]);
            if !source.is_null() {
                assert_eq!(
                    extra.track_id,
                    format!("/org/mpris/MediaPlayer2/track/{}", key.simple())
                );
                let duplicate = Metadata::for_item(&item, uuid::Uuid::new_v4(), None, None);
                assert_ne!(extra.track_id, duplicate.mpris.track_id);
            }
        }
        for (index, repeat) in [
            feishin_core::queue::Repeat::Off,
            feishin_core::queue::Repeat::All,
            feishin_core::queue::Repeat::One,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                loop_status(repeat).as_str(),
                reference["repeat"][index]["output"]
            );
        }
    }
}
