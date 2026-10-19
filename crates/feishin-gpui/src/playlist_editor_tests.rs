use crate::app::{Feishin, Route};
use feishin_core::{
    api::Api,
    model::{ItemKind, Server, ServerType},
    storage::State,
};
use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};
use gpui_component::Root;
use std::{
    cell::RefCell,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct Fixture {
    creates: Arc<AtomicUsize>,
    updates: Arc<AtomicUsize>,
    fail_create: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let result = self.worker.take().unwrap().join();
        if !std::thread::panicking() {
            result.unwrap();
        }
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn setup(cx: &mut TestAppContext) -> (Fixture, Entity<Feishin>, &mut VisualTestContext) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let creates = Arc::new(AtomicUsize::new(0));
    let updates = Arc::new(AtomicUsize::new(0));
    let fail_create = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let (created, updated, failure, stopped) = (
        creates.clone(),
        updates.clone(),
        fail_create.clone(),
        stop.clone(),
    );
    let worker = std::thread::spawn(move || {
        while !stopped.load(Ordering::SeqCst) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = vec![];
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 16_000);
            }
            let request = String::from_utf8(request).unwrap();
            let body = if request.contains("/rest/createPlaylist.view") {
                if failure.swap(false, Ordering::SeqCst) {
                    r#"{"subsonic-response":{"status":"failed","error":{"code":0,"message":"Try again"}}}"#.into()
                } else {
                    let id = created.fetch_add(1, Ordering::SeqCst) + 1;
                    format!(
                        r#"{{"subsonic-response":{{"status":"ok","playlist":{{"id":"{id}"}}}}}}"#
                    )
                }
            } else if request.contains("/rest/updatePlaylist.view") {
                updated.fetch_add(1, Ordering::SeqCst);
                r#"{"subsonic-response":{"status":"ok"}}"#.into()
            } else {
                r#"{"subsonic-response":{"status":"ok","playlists":{"playlist":[]},"albumList2":{"album":[]},"artists":{"index":[]}}}"#.into()
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        }
    });
    let server = Server {
        id: uuid::Uuid::new_v4(),
        name: "Playlist test".into(),
        url: format!("http://{address}"),
        username: "test".into(),
        kind: ServerType::Subsonic,
        legacy_auth: false,
        options: Default::default(),
    };
    let api = Api::new(server.clone(), Default::default()).unwrap();
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let dir = std::env::temp_dir().join(format!("feishin-playlist-save-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        let mut state = State::default();
        state.settings.home.items.clear();
        state.settings.home.feature = false;
        state.settings.lyric_fetch = false;
        state.settings.scrobble = false;
        state.settings.mpv_path = dir.join("absent-player").to_string_lossy().into();
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        app.update(cx, |app, cx| {
            app.state.servers.push(server.clone());
            app.state.active_server = Some(server.id);
            app.sessions.insert(server.id, api);
            app.edit_playlist(None, vec![], window, cx);
            app.form_name
                .update(cx, |input, cx| input.set_value("My playlist", window, cx));
        });
        captured.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    (
        Fixture {
            creates,
            updates,
            fail_create,
            stop,
            worker: Some(worker),
            dir,
        },
        app,
        cx,
    )
}

fn wait_for_save(app: &Entity<Feishin>, cx: &mut VisualTestContext) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        cx.run_until_parked();
        if app.read_with(cx, |app, _| !app.playlist_saving) {
            return;
        }
        assert!(Instant::now() < deadline, "Playlist save did not finish");
        std::thread::sleep(Duration::from_millis(3));
    }
}

#[gpui::test]
fn playlist_save_repeated_clicks_create_only_once(cx: &mut TestAppContext) {
    let (fixture, app, cx) = setup(cx);
    app.update(cx, |app, cx| {
        app.save_playlist(cx);
        assert!(app.playlist_saving);
        app.save_playlist(cx);
    });
    wait_for_save(&app, cx);
    assert_eq!(fixture.creates.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.updates.load(Ordering::SeqCst), 1);
    app.read_with(cx, |app, _| {
        assert_eq!(app.route, Route::Library(ItemKind::Playlist));
        assert_eq!(app.message, "Playlist saved");
    });
    // A queued click arriving after navigation must not create another playlist.
    app.update(cx, |app, cx| app.save_playlist(cx));
    cx.run_until_parked();
    assert_eq!(fixture.creates.load(Ordering::SeqCst), 1);
}

#[gpui::test]
fn playlist_save_completion_preserves_navigation(cx: &mut TestAppContext) {
    let (fixture, app, cx) = setup(cx);
    app.update(cx, |app, cx| {
        app.save_playlist(cx);
        app.navigate(Route::Settings, cx);
    });
    wait_for_save(&app, cx);
    assert_eq!(fixture.creates.load(Ordering::SeqCst), 1);
    assert_eq!(
        app.read_with(cx, |app, _| app.route.clone()),
        Route::Settings
    );
}

#[gpui::test]
fn playlist_save_completion_preserves_return_to_editor(cx: &mut TestAppContext) {
    let (_fixture, app, cx) = setup(cx);
    app.update(cx, |app, cx| {
        app.save_playlist(cx);
        app.back(cx);
        app.forward(cx);
    });
    wait_for_save(&app, cx);
    assert_eq!(
        app.read_with(cx, |app, _| app.route.clone()),
        Route::PlaylistEditor
    );
}

#[gpui::test]
fn playlist_save_completion_preserves_new_editor(cx: &mut TestAppContext) {
    let (_fixture, app, cx) = setup(cx);
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.save_playlist(cx);
            app.edit_playlist(None, vec![], window, cx);
            app.form_name.update(cx, |input, cx| {
                input.set_value("Another playlist", window, cx)
            });
        })
    });
    wait_for_save(&app, cx);
    app.read_with(cx, |app, cx| {
        assert_eq!(app.route, Route::PlaylistEditor);
        assert_eq!(app.form_name.read(cx).value().as_str(), "Another playlist");
    });
}

#[gpui::test]
fn playlist_save_failure_allows_retry(cx: &mut TestAppContext) {
    let (fixture, app, cx) = setup(cx);
    fixture.fail_create.store(true, Ordering::SeqCst);
    app.update(cx, |app, cx| app.save_playlist(cx));
    wait_for_save(&app, cx);
    app.read_with(cx, |app, cx| {
        assert_eq!(app.route, Route::PlaylistEditor);
        assert!(app.message.contains("Try again"));
        assert_eq!(app.form_name.read(cx).value().as_str(), "My playlist");
    });
    app.update(cx, |app, cx| app.save_playlist(cx));
    wait_for_save(&app, cx);
    assert_eq!(fixture.creates.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.updates.load(Ordering::SeqCst), 1);
    assert_eq!(
        app.read_with(cx, |app, _| app.route.clone()),
        Route::Library(ItemKind::Playlist)
    );
}

#[gpui::test]
fn playlist_save_completion_after_server_change_clears_busy_state(cx: &mut TestAppContext) {
    let (fixture, app, cx) = setup(cx);
    app.update(cx, |app, cx| {
        app.save_playlist(cx);
        app.state.active_server = None;
        app.navigate(Route::Servers, cx);
        app.message = "Different server".into();
    });
    wait_for_save(&app, cx);
    assert_eq!(fixture.creates.load(Ordering::SeqCst), 1);
    app.read_with(cx, |app, _| {
        assert_eq!(app.route, Route::Servers);
        assert_eq!(app.message, "Different server");
    });
}
