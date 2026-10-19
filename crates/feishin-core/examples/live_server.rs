//! Exercises the production client against disposable, local music servers.
//! scripts/native-tests/live-servers.py provisions and removes these fixtures.
use anyhow::{Context, Result, ensure};
use feishin_core::{
    api::{Api, PlaylistSelectionAction},
    model::{ItemKind, ListQuery, Server, ServerType},
    storage::Settings,
};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let kind: ServerType =
        serde_json::from_str(&format!("\"{}\"", args.next().context("Server kind")?))?;
    let address = args.next().context("Server URL")?;
    let url = url::Url::parse(&address)?;
    ensure!(
        url.scheme() == "http" && url.host_str() == Some("127.0.0.1"),
        "Only disposable localhost fixtures are accepted"
    );
    let username = args.next().context("Fixture username")?;
    let password = args.next().context("Fixture password")?;
    let server = Server {
        id: uuid::Uuid::new_v4(),
        name: "Native live fixture".into(),
        url: address,
        username,
        kind,
        legacy_auth: false,
        options: Default::default(),
    };
    let (api, _) = Api::authenticate(server, &password).context("Authentication")?;
    if api.server.kind == ServerType::Jellyfin {
        let quick_api = Api::new(
            Server {
                id: uuid::Uuid::new_v4(),
                username: String::new(),
                ..api.server.clone()
            },
            Default::default(),
        )?;
        let pending = quick_api
            .initiate_quick_connect()
            .context("Initiate Quick Connect")?;
        ensure!(
            !quick_api.quick_connect_authenticated(&pending)?,
            "Unauthenticated Quick Connect was already approved"
        );
        // Only the disposable fixture's account authorizes this generated code.
        let authorized: bool = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?
            .post(format!("{}/QuickConnect/Authorize", api.server.url))
            .query(&[("code", &pending.code)])
            .header(
                "Authorization",
                format!("MediaBrowser Token=\"{}\"", api.session().token),
            )
            .send()?
            .error_for_status()?
            .json()?;
        ensure!(
            authorized && quick_api.quick_connect_authenticated(&pending)?,
            "Fixture did not authorize Quick Connect"
        );
        let approved = quick_api.authenticate_quick_connect(&pending)?;
        ensure!(
            approved.server.username == api.server.username
                && approved.session().user_id == api.session().user_id
                && approved.session().is_admin == api.session().is_admin
                && !approved.session().token.is_empty(),
            "Quick Connect returned a different account"
        );
    }
    api.discover_features();
    let deadline = Instant::now() + Duration::from_secs(90);
    let songs = loop {
        let songs = api
            .all_items(ListQuery {
                kind: ItemKind::Song,
                ..Default::default()
            })
            .context("Song library")?;
        let fixture_metadata = songs.iter().all(|song| {
            song.name.starts_with("Native Track ") && song.album == "Native Parity Test"
        });
        if songs.len() == 3 && fixture_metadata {
            break songs;
        }
        ensure!(
            Instant::now() < deadline,
            "Library scan did not find the three fixture songs: {:?}",
            songs
                .iter()
                .map(|song| (&song.name, &song.album))
                .collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(500));
    };
    ensure!(
        songs
            .iter()
            .all(|song| song.duration > 1.9 && song.duration < 2.1 && !song.album_id.is_empty()),
        "Song metadata differs from generated fixtures"
    );
    for song in &songs {
        for album_artists in [false, true] {
            let artists = feishin_core::artists::links(song, album_artists);
            ensure!(
                !artists.is_empty()
                    && artists
                        .iter()
                        .all(|(id, name)| !id.is_empty() && name == "Native Test Artist"),
                "Artist link identities differ from generated fixtures: {artists:?}"
            );
        }
        let genres = feishin_core::genres::links(song);
        ensure!(
            genres.len() == 1 && !genres[0].0.is_empty() && genres[0].1 == "Ambient",
            "Genre link identities differ from generated fixtures: {genres:?}"
        );
        let subtitle = song.column_data.get("trackSubtitle").map(String::as_str);
        let expected = (api.server.kind == ServerType::Navidrome
            && !api.features().version.starts_with("0.54."))
        .then_some("Fixture movement");
        ensure!(
            subtitle == expected,
            "Track subtitle differs from generated fixture: {subtitle:?}"
        );
    }
    for kind in [
        ItemKind::Album,
        ItemKind::Artist,
        ItemKind::AlbumArtist,
        ItemKind::Genre,
    ] {
        let values = api
            .all_items(ListQuery {
                kind,
                ..Default::default()
            })
            .with_context(|| format!("{kind:?} library"))?;
        ensure!(!values.is_empty(), "No {kind:?} items");
    }
    for kind in [ItemKind::Song, ItemKind::Album, ItemKind::AlbumArtist] {
        for term in ["", "Native"] {
            let expected = api.all_items(ListQuery {
                kind,
                search: term.into(),
                ..Default::default()
            })?;
            let mut actual = Vec::new();
            loop {
                let page = api
                    .search(
                        &ListQuery {
                            kind,
                            search: term.into(),
                            offset: actual.len(),
                            limit: 1,
                            ..Default::default()
                        },
                        "",
                    )
                    .with_context(|| format!("{kind:?} paginated search"))?;
                ensure!(
                    page.items.iter().all(|item| item.kind == kind),
                    "Search mixed item types"
                );
                ensure!(page.items.len() <= 1, "Search ignored page size");
                let more = page.has_more;
                actual.extend(page.items.into_iter().map(|item| item.id));
                if !more {
                    break;
                }
                ensure!(
                    actual.len() <= expected.len(),
                    "Search pagination did not finish"
                );
            }
            ensure!(
                actual == expected.into_iter().map(|item| item.id).collect::<Vec<_>>(),
                "{kind:?} search paging skipped or repeated results"
            );
        }
        let missing = api.search(
            &ListQuery {
                kind,
                search: "absent-native-fixture-42".into(),
                ..Default::default()
            },
            "",
        )?;
        ensure!(
            missing.items.is_empty() && !missing.has_more,
            "No-match search returned items"
        );
    }
    // Tracks can become visible before the initial scan releases its database
    // transaction. Wait before exercising playlist and rating writes.
    let scan_deadline = Instant::now() + Duration::from_secs(60);
    while api
        .scan_status()
        .context("Initial library scan status")?
        .scanning
    {
        ensure!(
            Instant::now() < scan_deadline,
            "Initial fixture scan did not finish"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
    // Folder navigation must reach the same tracks, including album directories
    // on Jellyfin and ID3 index roots on Subsonic-compatible servers.
    let libraries = api.all_items(ListQuery {
        kind: ItemKind::Folder,
        ..Default::default()
    })?;
    let mut auto_dj_checks = 0;
    let mut selections = vec![Vec::new()];
    if let Some(library) = libraries.first() {
        selections.push(vec![library.id.clone()]);
    }
    for selected in selections {
        let mut server = api.server.clone();
        server.options.music_folder_ids = selected;
        let folders = Api::new(server, api.session())?;
        if !folders.server.options.music_folder_ids.is_empty() {
            use feishin_core::auto_dj::{Config, Mode, Strategy};
            for mode in [Mode::Songs, Mode::Albums] {
                for strategy in [Strategy::Similar, Strategy::LibraryRandom] {
                    let config = Config {
                        mode,
                        song_strategy: strategy,
                        album_strategy: strategy,
                        item_count: if mode == Mode::Songs { 2 } else { 1 },
                        ..Default::default()
                    };
                    let queue = if mode == Mode::Songs {
                        &songs[..1]
                    } else {
                        &[]
                    };
                    let generated = folders
                        .auto_dj(&songs[0], &config, queue)
                        .with_context(|| format!("Scoped Auto DJ {mode:?}/{strategy:?}"))?;
                    let mut actual = generated
                        .iter()
                        .map(|song| song.id.clone())
                        .collect::<Vec<_>>();
                    let mut expected = songs
                        .iter()
                        .skip(usize::from(mode == Mode::Songs))
                        .map(|song| song.id.clone())
                        .collect::<Vec<_>>();
                    actual.sort();
                    expected.sort();
                    ensure!(
                        actual == expected,
                        "Auto DJ {mode:?}/{strategy:?} changed the scoped tracks: {actual:?}"
                    );
                    auto_dj_checks += 1;
                }
            }
        }
        let mut pending = folders.folder_children(None).context("Root folder index")?;
        ensure!(!pending.is_empty(), "Folder index is empty");
        let mut visited = std::collections::HashSet::new();
        let mut found = std::collections::HashSet::new();
        while let Some(item) = pending.pop() {
            if item.kind == ItemKind::Folder {
                ensure!(
                    visited.insert(item.id.clone()),
                    "Repeated folder: {}",
                    item.name
                );
                ensure!(
                    visited.len() < 128,
                    "Fixture folder traversal did not finish"
                );
                pending.extend(
                    folders
                        .folder_children(Some(&item))
                        .with_context(|| format!("Folder {}", item.name))?,
                );
            } else if item.kind == ItemKind::Song {
                ensure!(
                    found.insert(item.id.clone()),
                    "Repeated folder track: {}",
                    item.name
                );
            }
        }
        ensure!(
            found
                == songs
                    .iter()
                    .map(|song| song.id.clone())
                    .collect::<std::collections::HashSet<_>>(),
            "Folder browsing lost library tracks: {found:?}"
        );
    }
    ensure!(
        auto_dj_checks == 4,
        "Auto DJ did not exercise a selected music library"
    );
    let album = api
        .item(&songs[0].album_id, ItemKind::Album)
        .context("Full album metadata")?;
    ensure!(
        album.name == "Native Parity Test"
            && !album.album_artist_ids.is_empty()
            && !album.genre_ids.is_empty(),
        "Album artist/genre metadata is incomplete: {:?}",
        (&album.name, &album.album_artist_ids, &album.genre_ids)
    );
    let artist = api
        .item(&album.album_artist_ids[0], ItemKind::AlbumArtist)
        .context("Full artist metadata")?;
    ensure!(
        artist.name == "Native Test Artist",
        "Artist detail metadata is incomplete"
    );
    if api.server.kind == ServerType::Navidrome {
        ensure!(
            artist
                .column_data
                .get("albumCount")
                .is_some_and(|count| count == "1")
                && artist
                    .column_data
                    .get("songCount")
                    .is_some_and(|count| count == "3"),
            "Artist role counts differ from the fixture album and songs: {:?}",
            artist.column_data
        );
    }
    for filters in [
        feishin_core::filters::Filters {
            artist_ids: vec![album.album_artist_ids[0].clone()],
            ..Default::default()
        },
        feishin_core::filters::Filters {
            genre_ids: vec![album.genre_ids[0].clone()],
            ..Default::default()
        },
    ] {
        let related = api
            .list(&ListQuery {
                kind: ItemKind::Album,
                filters,
                limit: 20,
                ..Default::default()
            })
            .context("Related album filters")?;
        ensure!(
            related.items.iter().any(|item| item.id == album.id),
            "Related album query missed the fixture album"
        );
    }
    let cover = api.cover_bytes(&songs[0]).context("Artwork")?;
    feishin_core::tags::artwork_mime(&cover).context("Artwork format")?;
    let server_lyrics = api.server_lyrics(&songs[0]).context("Server lyrics")?;
    let legacy_without_lyrics = api.server.kind == ServerType::Navidrome
        && api.features().version.starts_with("0.54.")
        || api.server.kind == ServerType::Subsonic && api.features().version.starts_with("0.16.");
    ensure!(
        server_lyrics.len() == if legacy_without_lyrics { 0 } else { 2 },
        "Lyric support differs from the pinned fixture version"
    );
    if !server_lyrics.is_empty() {
        ensure!(
            server_lyrics.len() == 2
                && server_lyrics[0].time == Some(0.)
                && server_lyrics[1].time == Some(1.)
                && server_lyrics[0].text == "Native fixture verse"
                && server_lyrics[1].text == "Native fixture chorus",
            "Server lyrics differ from the fixture: {server_lyrics:?}"
        );
    }
    let radio_mutations = api.server.kind == ServerType::Navidrome;
    if radio_mutations {
        api.create_radio(
            "Native fixture radio",
            "https://example.com/native-stream",
            "https://example.com/native-home",
        )
        .context("Create radio station")?;
        let station = api
            .all_items(ListQuery {
                kind: ItemKind::Radio,
                ..Default::default()
            })?
            .into_iter()
            .find(|item| item.name == "Native fixture radio")
            .context("Created radio station missing")?;
        let result = (|| -> Result<()> {
            ensure!(
                station.stream_url == "https://example.com/native-stream"
                    && station.homepage == "https://example.com/native-home",
                "Radio creation metadata differs"
            );
            api.update_radio(
                &station,
                "Native updated radio",
                "https://example.com/updated-stream",
                "https://example.com/updated-home",
            )
            .context("Update radio station")?;
            let updated = api
                .all_items(ListQuery {
                    kind: ItemKind::Radio,
                    ..Default::default()
                })?
                .into_iter()
                .find(|item| item.id == station.id)
                .context("Updated radio station missing")?;
            ensure!(
                updated.name == "Native updated radio"
                    && updated.stream_url == "https://example.com/updated-stream"
                    && updated.homepage == "https://example.com/updated-home",
                "Radio updates did not persist"
            );
            Ok(())
        })();
        let deleted = api.delete_radio(&station).context("Delete radio station");
        result.and(deleted)?;
        ensure!(
            !api.all_items(ListQuery {
                kind: ItemKind::Radio,
                ..Default::default()
            })?
            .iter()
            .any(|item| item.id == station.id),
            "Deleted radio station remains"
        );
    }
    let settings = Settings {
        transcode: false,
        ..Default::default()
    };
    let stream = api.stream_url(&songs[0], &settings).context("Stream URL")?;
    let mut audio = vec![];
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?
        .get(&stream)
        .send()
        .context("Stream request")?
        .error_for_status()?
        .take(1024 * 1024)
        .read_to_end(&mut audio)?;
    ensure!(
        audio.starts_with(b"fLaC"),
        "Original audio stream is not the fixture FLAC"
    );
    for bitrate in [None, Some(257)] {
        let settings = Settings {
            transcode: true,
            bitrate,
            format: "mp3".into(),
            max_sample_rate: Some(44100),
            ..Default::default()
        };
        let stream = api.stream_url(&songs[0], &settings)?;
        let mut audio = vec![];
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?
            .get(stream)
            .send()
            .context("Transcoded stream request")?
            .error_for_status()?
            .take(1024 * 1024)
            .read_to_end(&mut audio)?;
        ensure!(
            !audio.is_empty(),
            "Empty transcoded stream for bitrate {bitrate:?}"
        );
        let ffmpeg = std::env::var_os("FEISHIN_TEST_FFMPEG").unwrap_or_else(|| "ffmpeg".into());
        let mut decode = Command::new(ffmpeg)
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
                "pipe:0",
                "-f",
                "null",
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        decode.stdin.take().unwrap().write_all(&audio)?;
        let result = decode.wait_with_output()?;
        ensure!(
            result.status.success(),
            "Transcoded stream decode failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    api.favorite(&songs[0], true).context("Favorite")?;
    ensure!(
        api.item(&songs[0].id, ItemKind::Song)?.favorite,
        "Favorite did not persist"
    );
    api.favorite(&songs[0], false)?;
    if api.server.kind != ServerType::Jellyfin {
        api.rate(&songs[0], 4).context("Rating")?;
        ensure!(
            api.item(&songs[0].id, ItemKind::Song)?.rating == 4,
            "Rating did not persist"
        );
        api.rate(&songs[0], 0)?;
    }
    let order = |items: &[feishin_core::model::Item]| {
        items.iter().map(|item| item.id.clone()).collect::<Vec<_>>()
    };
    let original = vec![songs[0].clone(), songs[1].clone(), songs[0].clone()];
    let id = api
        .create_playlist_checked("Native fixture playlist", false, None, &original)
        .context("Create duplicate playlist")?;
    // The older pinned Navidrome fixture predates native image editing.
    let image_mutations = api.server.kind == ServerType::Jellyfin
        || api.server.kind == ServerType::Navidrome && !api.features().version.starts_with("0.54.");
    let result = (|| -> Result<()> {
        let mut playlist = api
            .item(&id, ItemKind::Playlist)
            .context("Playlist metadata")?;
        if image_mutations {
            api.upload_image(&playlist, cover.clone())
                .context("Upload playlist artwork")?;
            let uploaded = api.item(&id, ItemKind::Playlist)?;
            let image = api
                .cover_bytes(&uploaded)
                .context("Read uploaded playlist artwork")?;
            feishin_core::tags::artwork_mime(&image).context("Uploaded artwork format")?;
            ensure!(!image.is_empty(), "Uploaded image is empty");
            api.remove_image(&uploaded)
                .context("Remove playlist artwork")?;
        }
        let tracks = api
            .all_detail_items(&playlist)
            .context("Playlist entries")?;
        ensure!(
            order(&tracks) == order(&original),
            "Playlist duplicates/order were lost"
        );
        playlist.owner_id = api.session().user_id;
        api.update_playlist_checked(
            &playlist,
            "Native renamed playlist",
            "Native fixture comment",
            true,
            None,
        )
        .context("Update playlist")?;
        let updated = api.item(&id, ItemKind::Playlist)?;
        ensure!(
            updated.name == "Native renamed playlist"
                && updated.comment == "Native fixture comment",
            "Playlist edits did not persist"
        );
        // Older Jellyfin regenerates entry IDs while saving playlist metadata.
        // The UI reloads after saving; exercise subsequent edits with that view.
        playlist = updated;
        let tracks = api.all_detail_items(&playlist)?;
        ensure!(
            order(&tracks) == order(&original),
            "Playlist metadata changed the track order"
        );
        api.edit_playlist_selection_checked(
            &playlist,
            &tracks,
            &[1],
            PlaylistSelectionAction::MoveBefore(Some(0)),
        )
        .context("Move selected entry")?;
        let tracks = api.all_detail_items(&playlist)?;
        ensure!(
            order(&tracks) == order(&[songs[1].clone(), songs[0].clone(), songs[0].clone()]),
            "Selected move changed duplicates: {:?}",
            order(&tracks)
        );
        api.edit_playlist_selection_checked(
            &playlist,
            &tracks,
            &[0],
            PlaylistSelectionAction::MoveBefore(None),
        )
        .context("Move first entry to end")?;
        let tracks = api.all_detail_items(&playlist)?;
        ensure!(
            order(&tracks) == order(&[songs[0].clone(), songs[0].clone(), songs[1].clone()]),
            "Downward move changed duplicates"
        );
        api.edit_playlist_selection_checked(
            &playlist,
            &tracks,
            &[1],
            PlaylistSelectionAction::Remove,
        )
        .context("Remove selected duplicate")?;
        let removed = api.all_detail_items(&playlist)?;
        ensure!(
            order(&removed) == order(&[songs[0].clone(), songs[1].clone()]),
            "Selected removal changed the wrong occurrence: before {:?}, removed {:?}, after {:?}",
            order(&tracks),
            tracks[1].playlist_entry_id,
            order(&removed)
        );
        let replacement = [songs[0].clone(), songs[0].clone(), songs[2].clone()];
        api.replace_playlist_tracks_checked(&playlist, &replacement)
            .context("Replace playlist")?;
        ensure!(
            order(&api.all_detail_items(&playlist)?) == order(&replacement),
            "Playlist replacement lost duplicates"
        );
        api.add_to_playlist_checked(&id, &songs[1..2])?;
        let appended = api.all_detail_items(&playlist)?;
        ensure!(
            appended.len() == 4,
            "Playlist append did not persist: {:?}",
            order(&appended)
        );
        if api.server.kind != ServerType::Jellyfin {
            api.save_queue(&original, Some(2), 1.25)
                .context("Save duplicate server queue")?;
            let (restored, current, position) = api.load_queue().context("Restore server queue")?;
            let index_supported = api.features().index_queue || api.features().native_queue;
            ensure!(
                order(&restored) == order(&original)
                    && current == Some(if index_supported { 2 } else { 0 })
                    && (position - 1.25).abs() < 0.01,
                "Server queue differs from its advertised capability: current {current:?}, position {position}"
            );
        }
        Ok(())
    })();
    let deleted = api.delete_playlist(&id).context("Delete fixture playlist");
    result.and(deleted)?;
    ensure!(
        !api.all_items(ListQuery {
            kind: ItemKind::Playlist,
            ..Default::default()
        })?
        .iter()
        .any(|playlist| playlist.id == id),
        "Deleted playlist is still present"
    );
    api.start_scan().context("Rescan fixture music library")?;
    let deadline = Instant::now() + Duration::from_secs(90);
    while api.scan_status().context("Library scan status")?.scanning {
        ensure!(Instant::now() < deadline, "Fixture rescan did not finish");
        std::thread::sleep(Duration::from_millis(500));
    }
    println!(
        "{}",
        serde_json::json!({"backend":api.server.kind, "version":api.features().version, "songs":songs.len(), "artwork_bytes":cover.len(), "stream_bytes":audio.len(), "server_lyric_lines":server_lyrics.len(), "radio_mutations":radio_mutations, "image_mutations":image_mutations, "duplicate_playlist":true, "quick_connect":api.server.kind == ServerType::Jellyfin, "server_queue":api.server.kind != ServerType::Jellyfin, "queue_current_index":api.features().index_queue || api.features().native_queue, "library_scan":true, "auto_dj_checks":auto_dj_checks, "transcoding_checks":2})
    );
    Ok(())
}
