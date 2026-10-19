//! Complete native playlist edits after Jellyfin's asynchronous metadata saves.
use crate::{
    api::{Api, PlaylistSelectionAction, playlist_moves},
    model::{Item, ItemKind, ServerType},
};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};

pub(crate) type WriteLocks = Arc<Mutex<HashMap<String, Weak<Mutex<()>>>>>;

impl Api {
    fn playlist_write_lock(&self, id: &str) -> Result<Arc<Mutex<()>>> {
        let mut locks = self
            .playlist_writes
            .lock()
            .map_err(|_| anyhow::anyhow!("Playlist write lock failed"))?;
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(id).and_then(Weak::upgrade) {
            return Ok(lock);
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(id.into(), Arc::downgrade(&lock));
        Ok(lock)
    }

    fn playlist_stub(&self, id: &str) -> Item {
        Item {
            id: id.into(),
            server_id: self.server.id,
            kind: ItemKind::Playlist,
            ..Default::default()
        }
    }

    fn verify_playlist_order(&self, id: &str, expected: &[String]) -> Result<()> {
        self.verify_playlist_order_for(
            id,
            expected,
            Duration::from_millis(500),
            Duration::from_secs(5),
        )
    }

    fn verify_playlist_order_for(
        &self,
        id: &str,
        expected: &[String],
        quiet: Duration,
        timeout: Duration,
    ) -> Result<()> {
        let playlist = self.playlist_stub(id);
        let deadline = Instant::now() + timeout;
        let mut previous = None;
        let mut stable_since = Instant::now();
        loop {
            let entries = self.all_tracks(&playlist)?;
            let identities = entries
                .iter()
                .map(|entry| (entry.id.clone(), entry.playlist_entry_id.clone()))
                .collect::<Vec<_>>();
            let matches = entries.iter().map(|entry| &entry.id).eq(expected.iter());
            if matches && previous.as_ref() == Some(&identities) {
                if stable_since.elapsed() >= quiet {
                    return Ok(());
                }
            } else {
                stable_since = Instant::now();
            }
            previous = Some(identities);
            ensure!(
                Instant::now() < deadline,
                "Jellyfin did not retain the requested playlist order. Refresh the playlist before editing again."
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn create_playlist_checked(
        &self,
        name: &str,
        public: bool,
        rules: Option<Value>,
        songs: &[Item],
    ) -> Result<String> {
        if self.server.kind != ServerType::Jellyfin {
            return self.create_playlist(name, public, rules, songs);
        }
        ensure!(
            songs
                .iter()
                .all(|song| song.server_id == self.server.id && song.kind == ItemKind::Song),
            "Playlist tracks must belong to the same server"
        );
        let id = self.create_playlist(name, public, rules, &[])?;
        self.add_to_playlist_checked(&id, songs).with_context(|| {
            format!("Created playlist {id}, but could not finish adding its tracks")
        })?;
        Ok(id)
    }

    pub fn add_to_playlist_checked(&self, id: &str, songs: &[Item]) -> Result<()> {
        if self.server.kind != ServerType::Jellyfin {
            return self.add_to_playlist(id, songs);
        }
        ensure!(
            songs
                .iter()
                .all(|song| song.server_id == self.server.id && song.kind == ItemKind::Song),
            "Playlist tracks must belong to the same server"
        );
        let lock = self.playlist_write_lock(id)?;
        let _guard = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Playlist write lock failed"))?;
        let mut expected = self
            .all_tracks(&self.playlist_stub(id))?
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        for chunk in songs.chunks(50) {
            self.add_to_playlist(id, chunk)?;
            expected.extend(chunk.iter().map(|song| song.id.clone()));
            self.verify_playlist_order(id, &expected)?;
        }
        if songs.is_empty() {
            self.verify_playlist_order(id, &expected)?;
        }
        Ok(())
    }

    pub fn update_playlist_checked(
        &self,
        playlist: &Item,
        name: &str,
        comment: &str,
        public: bool,
        rules: Option<Value>,
    ) -> Result<()> {
        if self.server.kind != ServerType::Jellyfin {
            return self.update_playlist(playlist, name, comment, public, rules);
        }
        ensure!(!name.trim().is_empty(), "Enter a playlist name");
        ensure!(
            playlist.kind == ItemKind::Playlist && playlist.server_id == self.server.id,
            "Playlist belongs to a different server"
        );
        let lock = self.playlist_write_lock(&playlist.id)?;
        let _guard = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Playlist write lock failed"))?;
        let expected = self
            .all_tracks(&self.playlist_stub(&playlist.id))?
            .into_iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        self.update_playlist(playlist, name, comment, public, rules)?;
        self.verify_playlist_order(&playlist.id, &expected)?;
        let saved = self.item(&playlist.id, ItemKind::Playlist)?;
        ensure!(
            saved.name == name && saved.comment == comment,
            "Jellyfin did not retain the playlist metadata. Refresh the playlist before editing again."
        );
        Ok(())
    }

    pub fn replace_playlist_tracks_checked(&self, playlist: &Item, songs: &[Item]) -> Result<()> {
        if self.server.kind != ServerType::Jellyfin {
            return self.replace_playlist_tracks(playlist, songs);
        }
        let lock = self.playlist_write_lock(&playlist.id)?;
        let _guard = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Playlist write lock failed"))?;
        self.replace_playlist_tracks(playlist, songs)?;
        self.verify_playlist_order(
            &playlist.id,
            &songs.iter().map(|song| song.id.clone()).collect::<Vec<_>>(),
        )
    }

    pub fn edit_playlist_selection_checked(
        &self,
        playlist: &Item,
        expected: &[Item],
        selected: &[usize],
        action: PlaylistSelectionAction,
    ) -> Result<()> {
        if self.server.kind != ServerType::Jellyfin {
            return self.edit_playlist_selection(playlist, expected, selected, action);
        }
        ensure!(
            playlist.kind == ItemKind::Playlist && playlist.server_id == self.server.id,
            "Playlist belongs to a different server"
        );
        ensure!(
            !selected.is_empty() && selected.len() <= 500,
            "Select 1 to 500 playlist tracks"
        );
        let lock = self.playlist_write_lock(&playlist.id)?;
        let _guard = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Playlist write lock failed"))?;
        let tracks = self.all_tracks(&self.playlist_stub(&playlist.id))?;
        let selected = selected.iter().copied().collect::<BTreeSet<_>>();
        let mut order = tracks
            .iter()
            .map(|track| track.id.clone())
            .collect::<Vec<_>>();
        match action {
            PlaylistSelectionAction::Remove => {
                order = order
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, id)| (!selected.contains(&index)).then_some(id))
                    .collect();
            }
            PlaylistSelectionAction::MoveBefore(before) => {
                for (from, to) in playlist_moves(tracks.len(), &selected, before)? {
                    let id = order.remove(from);
                    order.insert(to, id);
                }
            }
        }
        self.edit_playlist_selection(
            playlist,
            expected,
            &selected.into_iter().collect::<Vec<_>>(),
            action,
        )?;
        self.verify_playlist_order(&playlist.id, &order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Server, Session};
    use serde_json::json;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::{AtomicBool, Ordering},
        thread,
    };

    #[test]
    fn jellyfin_writes_wait_through_transient_state_serialize_appends_and_never_repeat_a_write() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Playlist fixture".into(),
            url: format!("http://{}", listener.local_addr().unwrap()),
            username: "fixture".into(),
            kind: ServerType::Jellyfin,
            legacy_auth: false,
            options: Default::default(),
        };
        let api = Api::new(
            server,
            Session {
                user_id: "fixture".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let corrupt = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::<String>::new()));
        let saved = Arc::new(Mutex::new(Vec::<String>::new()));
        let (stop, drop_writes, calls, state) = (
            done.clone(),
            corrupt.clone(),
            requests.clone(),
            saved.clone(),
        );
        let worker = thread::spawn(move || {
            let mut revision = 0;
            let mut transient = 0;
            while !stop.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let mut buffer = [0; 4096];
                loop {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|value| value.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                let first = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .collect::<Vec<_>>();
                let url = url::Url::parse(&format!("http://localhost{}", first[1])).unwrap();
                let response = if first[0] == "POST" {
                    if !drop_writes.load(Ordering::Relaxed) {
                        if url.path() == "/Playlists/p" {
                            let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
                            *state.lock().unwrap() = body["Ids"].as_array().unwrap().iter().map(|value| value.as_str().unwrap().into()).collect();
                            transient = 2;
                        } else {
                            assert_eq!(url.path(), "/Playlists/p/Items");
                            let ids = url.query_pairs().find(|(key,_)| key=="Ids").unwrap().1;
                            state.lock().unwrap().extend(ids.split(',').map(str::to_owned));
                        }
                        revision += 1;
                    }
                    Value::Null
                } else {
                    assert_eq!(first[0],"GET");
                    assert_eq!(url.path(),"/Playlists/p/Items");
                    let mut ids = state.lock().unwrap().clone();
                    if transient > 0 {
                        // A valid read followed by stale metadata must not finish
                        // the edit before the actual server state stabilizes.
                        if transient == 1 { ids = vec!["stale".into()]; }
                        transient -= 1;
                        revision += 1;
                    }
                    json!({"Items":ids.iter().enumerate().map(|(index,id)|json!({"Id":id,"Name":id,"Type":"Audio","PlaylistItemId":format!("{revision}-{index}")})).collect::<Vec<_>>(),"TotalRecordCount":ids.len()})
                }.to_string();
                calls.lock().unwrap().push(request);
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).unwrap();
            }
        });
        let playlist = api.playlist_stub("p");
        let song = |id: &str| Item {
            id: id.into(),
            server_id: api.server.id,
            ..Default::default()
        };
        let before = Instant::now();
        api.replace_playlist_tracks_checked(&playlist, &[song("a"), song("a"), song("b")])
            .unwrap();
        assert!(before.elapsed() >= Duration::from_millis(500));
        assert_eq!(*saved.lock().unwrap(), ["a", "a", "b"]);
        let first_api = api.clone();
        let first_song = song("x");
        let first = thread::spawn(move || first_api.add_to_playlist_checked("p", &[first_song]));
        let second_api = api.clone();
        let second_song = song("y");
        let second = thread::spawn(move || second_api.add_to_playlist_checked("p", &[second_song]));
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
        let tracks = saved.lock().unwrap().clone();
        assert_eq!(&tracks[..3], ["a", "a", "b"]);
        assert!(tracks[3..] == ["x", "y"] || tracks[3..] == ["y", "x"]);
        let many = (0..51)
            .map(|index| song(&format!("bulk-{index}")))
            .collect::<Vec<_>>();
        api.add_to_playlist_checked("p", &many).unwrap();
        assert_eq!(saved.lock().unwrap().len(), 56);
        let mut invalid = many.clone();
        invalid[50].server_id = uuid::Uuid::new_v4();
        let before = requests.lock().unwrap().len();
        assert!(api.add_to_playlist_checked("p", &invalid).is_err());
        assert!(
            api.create_playlist_checked("Invalid", false, None, &invalid)
                .is_err()
        );
        assert!(
            api.update_playlist_checked(&playlist, "", "", false, None)
                .is_err()
        );
        assert!(
            api.edit_playlist_selection_checked(
                &playlist,
                &[],
                &[],
                PlaylistSelectionAction::Remove
            )
            .is_err()
        );
        assert_eq!(requests.lock().unwrap().len(), before);
        let prior_posts = requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with("POST "))
            .count();
        assert_eq!(prior_posts, 5);
        corrupt.store(true, Ordering::Relaxed);
        api.replace_playlist_tracks(&playlist, &[song("missing")])
            .unwrap();
        let error = api
            .verify_playlist_order_for(
                "p",
                &["missing".into()],
                Duration::from_millis(50),
                Duration::from_millis(200),
            )
            .unwrap_err();
        assert!(error.to_string().contains("Refresh the playlist"));
        assert_eq!(saved.lock().unwrap().len(), 56);
        assert_eq!(
            requests
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request.starts_with("POST "))
                .count(),
            prior_posts + 1
        );
        done.store(true, Ordering::Relaxed);
        worker.join().unwrap();
    }
}
