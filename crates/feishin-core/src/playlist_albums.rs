use crate::model::{Item, ItemKind, Sort};

// Match playlistSongsToAlbums: repeated, nonadjacent albums stay separate.
pub fn rows(songs: Vec<Item>) -> Vec<Item> {
    let mut rows: Vec<Item> = vec![];
    for song in songs {
        if let Some(row) = rows.last_mut()
            && row.server_id == song.server_id
            && row.id == song.album_id
        {
            row.playlist_songs.push(song);
            continue;
        }
        let mut album = Item {
            id: song.album_id.clone(),
            name: song.album.clone(),
            kind: ItemKind::Album,
            track: 0,
            disc: 0,
            duration: 0.,
            favorite: false,
            rating: 0,
            play_count: 0,
            resume_position: 0.,
            resumable: false,
            stream_url: String::new(),
            path: String::new(),
            container: String::new(),
            musicbrainz_id: String::new(),
            musicbrainz_release_group_id: String::new(),
            gain_track: None,
            gain_album: None,
            peak_track: None,
            peak_album: None,
            ..song.clone()
        };
        album.column_data.retain(|key, _| {
            matches!(
                key.as_str(),
                "releaseYear" | "releaseDate" | "originalYear" | "originalDate" | "explicitStatus"
            )
        });
        album.playlist_songs = vec![song];
        rows.push(album);
    }
    rows
}

pub fn ordered_tracks(album: &Item) -> Vec<Item> {
    let mut songs = album.playlist_songs.clone();
    crate::lists::sort_items(&mut songs, Sort::Track, false);
    songs
}

pub fn sort_songs(songs: &mut [Item], sort: Sort, descending: bool) {
    crate::lists::sort_items(songs, sort, descending);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_groups_keep_duplicate_entries_disc_order_and_source_positions() {
        let songs = ["a", "a", "b", "a", "", ""]
            .into_iter()
            .enumerate()
            .map(|(i, album)| Item {
                id: if i == 1 {
                    "song-0".into()
                } else {
                    format!("song-{i}")
                },
                album_id: album.into(),
                album: format!("Album {album}"),
                disc: 1,
                track: if i == 0 { 3 } else { 1 },
                playlist_entry_id: format!("entry-{i}"),
                playlist_position: Some(i),
                favorite: true,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let groups = rows(songs.clone());
        let mut original_order = songs.clone();
        sort_songs(&mut original_order, Sort::Id, false);
        assert_eq!(original_order, songs);
        sort_songs(&mut original_order, Sort::Id, true);
        assert_eq!(
            original_order,
            songs.iter().rev().cloned().collect::<Vec<_>>()
        );
        assert_eq!(
            groups.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "a", ""]
        );
        assert_eq!(
            groups
                .iter()
                .map(|row| row.playlist_songs.len())
                .collect::<Vec<_>>(),
            [2, 1, 1, 2]
        );
        assert_eq!(
            groups
                .iter()
                .flat_map(Item::action_items)
                .collect::<Vec<_>>(),
            songs
        );
        assert_eq!(
            ordered_tracks(&groups[0])
                .iter()
                .map(|song| song.playlist_entry_id.as_str())
                .collect::<Vec<_>>(),
            ["entry-1", "entry-0"]
        );
        assert_eq!(groups[2].playlist_position, Some(3));
        assert!(
            groups
                .iter()
                .all(|row| row.kind == ItemKind::Album && !row.favorite && row.rating == 0)
        );
        let restored: Vec<Item> =
            serde_json::from_slice(&serde_json::to_vec(&groups).unwrap()).unwrap();
        assert_eq!(restored, groups);
        let preview = crate::migration::preview(
            &crate::storage::Settings::default(),
            br#"{"general":{"playlistTarget":"album"}}"#,
        )
        .unwrap();
        assert!(preview.settings.playlist_albums);
        assert!(preview.omitted.is_empty());
        assert!(
            crate::migration::preview(
                &preview.settings,
                br#"{"general":{"playlistTarget":"bad"}}"#
            )
            .is_err()
        );
    }
}
