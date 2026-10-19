use crate::{
    api::Api,
    model::{Item, ItemKind},
    storage::Settings,
};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub enum Command<'a> {
    Get,
    Status,
    Start,
    Stop,
    Clear,
    Set(&'a [&'a str]),
    Add(&'a str),
    Remove(usize),
    Skip(usize, f64),
    Gain(f32),
}
impl Command<'_> {
    pub(crate) fn query(&self) -> Result<Vec<(String, String)>> {
        let (action, mut args) = match self {
            Self::Get => ("get", vec![]),
            Self::Status => ("status", vec![]),
            Self::Start => ("start", vec![]),
            Self::Stop => ("stop", vec![]),
            Self::Clear => ("clear", vec![]),
            Self::Set(ids) => {
                ensure!(ids.len() <= 100_000, "Too many Jukebox tracks");
                (
                    "set",
                    ids.iter().map(|id| ("id".into(), (*id).into())).collect(),
                )
            }
            Self::Add(id) => ("add", vec![("id".into(), (*id).into())]),
            Self::Remove(index) => ("remove", vec![("index".into(), index.to_string())]),
            Self::Skip(index, position) => {
                ensure!(
                    position.is_finite() && *position >= 0. && *position < u64::MAX as f64,
                    "Invalid Jukebox position"
                );
                (
                    "skip",
                    vec![
                        ("index".into(), index.to_string()),
                        ("offset".into(), position.floor().to_string()),
                    ],
                )
            }
            Self::Gain(gain) => {
                ensure!(
                    gain.is_finite() && (0. ..=1.).contains(gain),
                    "Invalid Jukebox gain"
                );
                ("setGain", vec![("gain".into(), gain.to_string())])
            }
        };
        ensure!(
            args.iter()
                .filter(|(name, _)| name == "id")
                .all(|(_, id)| !id.is_empty() && id.len() <= 4096),
            "Invalid Jukebox track ID"
        );
        args.insert(0, ("action".into(), action.into()));
        Ok(args)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub items: Vec<Item>,
    pub index: usize,
    pub position: f64,
    pub gain: f32,
    pub playing: bool,
}
impl Snapshot {
    pub(crate) fn parse(api: &Api, value: &Value) -> Result<Option<Self>> {
        let Some(state) = value
            .get("jukeboxPlaylist")
            .or_else(|| value.get("jukeboxStatus"))
        else {
            return Ok(None);
        };
        ensure!(state.is_object(), "Invalid Jukebox state");
        let gain = state
            .get("gain")
            .map_or(Ok(1.), |v| v.as_f64().context("Invalid Jukebox gain"))?;
        let position = state
            .get("position")
            .map_or(Ok(0.), |v| v.as_f64().context("Invalid Jukebox position"))?;
        ensure!(
            gain.is_finite()
                && (0. ..=1.).contains(&gain)
                && position.is_finite()
                && position >= 0.,
            "Invalid Jukebox state"
        );
        let entries = match state.get("entry").filter(|v| !v.is_null()) {
            Some(entries) => entries
                .as_array()
                .context("Invalid Jukebox playlist")?
                .as_slice(),
            None => &[],
        };
        ensure!(entries.len() <= 100_000, "Too many Jukebox tracks");
        let items = entries
            .iter()
            .map(|entry| api.normalize(entry, ItemKind::Song))
            .collect::<Vec<_>>();
        ensure!(
            items
                .iter()
                .all(|item| !item.id.is_empty() && item.id.len() <= 4096),
            "Invalid Jukebox track ID"
        );
        let index = state
            .get("currentIndex")
            .map_or(Ok(0), |v| v.as_i64().context("Invalid Jukebox index"))?;
        ensure!(
            index >= 0 || (items.is_empty() && index == -1),
            "Invalid Jukebox index"
        );
        let index = usize::try_from(index.max(0)).context("Invalid Jukebox index")?;
        ensure!(
            items.is_empty() || index < items.len(),
            "Jukebox index is outside the playlist"
        );
        Ok(Some(Self {
            items,
            index,
            gain: gain as f32,
            position,
            playing: state.get("playing").map_or(Ok(false), |v| {
                v.as_bool().context("Invalid Jukebox playing state")
            })?,
        }))
    }
    pub fn current(&self) -> Option<&Item> {
        self.items.get(self.index)
    }
}

pub(crate) struct Engine {
    pub api: Api,
    pub state: Snapshot,
    pub next_key: String,
    advanced: bool,
    stopped: bool,
    next_synced: bool,
}
impl Engine {
    pub fn new(api: Api) -> Result<Self> {
        let state = api
            .jukebox(Command::Get)?
            .context("Server returned no Jukebox playlist")?;
        Ok(Self {
            api,
            state,
            next_key: String::new(),
            advanced: false,
            stopped: false,
            next_synced: false,
        })
    }
    pub fn load(
        &mut self,
        item: &Item,
        key: &str,
        position: f64,
        settings: &Settings,
    ) -> Result<()> {
        ensure!(
            item.kind == ItemKind::Song && item.server_id == self.api.server.id,
            "Jukebox tracks must belong to the selected server"
        );
        if self.advanced
            && self.next_key == key
            && self
                .state
                .current()
                .is_some_and(|track| track.id == item.id)
        {
            for _ in 0..self.state.index {
                self.api.jukebox(Command::Remove(0))?;
            }
            self.state.items.drain(..self.state.index);
            self.state.index = 0;
        } else {
            self.api.jukebox(Command::Set(&[&item.id]))?;
            self.api.jukebox(Command::Skip(0, position))?;
            self.state.items = vec![item.clone()];
            self.state.index = 0;
            self.state.position = position.floor();
        }
        self.advanced = false;
        self.next_key.clear();
        self.next_synced = false;
        self.stopped = false;
        self.volume(settings)?;
        self.api.jukebox(Command::Start)?;
        self.state.playing = true;
        Ok(())
    }
    pub fn next(&mut self, item: &Item, key: String) -> Result<()> {
        if (self.next_synced && self.next_key == key) || self.advanced {
            return Ok(());
        }
        ensure!(
            key.is_empty() || (item.kind == ItemKind::Song && item.server_id == self.api.server.id),
            "Jukebox tracks must belong to the selected server"
        );
        let state = self
            .api
            .jukebox(Command::Get)?
            .context("Server returned no Jukebox playlist")?;
        // Retain the current server index and position, including a recovered queue.
        for index in ((state.index + 1)..state.items.len()).rev() {
            self.api.jukebox(Command::Remove(index))?;
        }
        if !key.is_empty() {
            self.api.jukebox(Command::Add(&item.id))?;
        }
        self.state = state;
        self.state.items.truncate(self.state.index + 1);
        if !key.is_empty() {
            self.state.items.push(item.clone());
        }
        self.next_key = key;
        self.next_synced = true;
        Ok(())
    }
    pub fn pause(&mut self, paused: bool) -> Result<()> {
        self.api.jukebox(if paused {
            Command::Stop
        } else {
            Command::Start
        })?;
        self.state.playing = !paused;
        self.stopped = false;
        Ok(())
    }
    pub fn seek(&mut self, position: f64) -> Result<()> {
        self.api
            .jukebox(Command::Skip(self.state.index, position))?;
        self.state.position = position.floor();
        Ok(())
    }
    pub fn volume(&mut self, settings: &Settings) -> Result<()> {
        let gain = if settings.muted {
            0.
        } else {
            settings.volume / 100.
        };
        self.api.jukebox(Command::Gain(gain))?;
        self.state.gain = gain;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.api.jukebox(Command::Stop)?;
        self.api.jukebox(Command::Clear)?;
        self.state = Snapshot {
            gain: self.state.gain,
            ..Default::default()
        };
        self.next_key.clear();
        self.next_synced = false;
        self.stopped = true;
        Ok(())
    }
    /// True when the server advanced into the prepared next entry or exhausted its queue.
    pub fn poll(&mut self) -> Result<bool> {
        let state = self
            .api
            .jukebox(Command::Get)?
            .context("Server returned no Jukebox playlist")?;
        let advanced = !self.next_key.is_empty()
            && state.playing
            && state.index == self.state.index + 1
            && state
                .current()
                .zip(self.state.items.get(self.state.index + 1))
                .is_some_and(|(a, b)| a.id == b.id);
        let duration = self.state.current().map_or(0., |item| item.duration);
        let ended = !self.stopped
            && self.state.playing
            && !state.playing
            && self.state.position > 0.
            && (state.items.is_empty()
                || state.position == 0.
                || (duration > 0. && state.position >= duration - 1.));
        self.state = state;
        if advanced {
            self.advanced = true;
        }
        if ended {
            self.stopped = true;
        }
        Ok(advanced || ended)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Server, ServerType, Session},
        playback::{Control, Event, Player},
    };
    use serde_json::json;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    type Query = Vec<(String, String)>;
    struct Fixture {
        api: Api,
        state: Arc<Mutex<Value>>,
        queries: Arc<Mutex<Vec<Query>>>,
        done: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }
    impl Fixture {
        fn new(kind: ServerType) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let api = Api::new(
                Server {
                    id: uuid::Uuid::new_v4(),
                    name: "Jukebox fixture".into(),
                    url: format!("http://{}/music", listener.local_addr().unwrap()),
                    username: "fixture".into(),
                    kind,
                    legacy_auth: false,
                    options: Default::default(),
                },
                Session::default(),
            )
            .unwrap();
            let state = Arc::new(Mutex::new(
                json!({"currentIndex":1,"playing":true,"position":7,"gain":0.23,"entry":[
                    {"id":"duplicate","title":"First","duration":20},
                    {"id":"duplicate","title":"Second","duration":20},
                    {"id":"other","title":"Other","duration":20}
                ]}),
            ));
            let queries = Arc::new(Mutex::new(vec![]));
            let done = Arc::new(AtomicBool::new(false));
            let (worker_state, worker_queries, worker_done) =
                (state.clone(), queries.clone(), done.clone());
            let worker = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(60);
                while !worker_done.load(Ordering::SeqCst) && Instant::now() < deadline {
                    let Ok((mut stream, _)) = listener.accept() else {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut first = String::new();
                    reader.read_line(&mut first).unwrap();
                    loop {
                        let mut header = String::new();
                        reader.read_line(&mut header).unwrap();
                        if header == "\r\n" || header.is_empty() {
                            break;
                        }
                    }
                    let path = first.split_whitespace().nth(1).unwrap();
                    let url = url::Url::parse(&format!("http://fixture{path}")).unwrap();
                    let args = url
                        .query_pairs()
                        .map(|(k, v)| (k.into_owned(), v.into_owned()))
                        .collect::<Vec<_>>();
                    let parameter = |key: &str| {
                        args.iter()
                            .find(|(name, _)| name == key)
                            .map(|(_, value)| value.as_str())
                            .unwrap_or_default()
                    };
                    let response = if url.path().ends_with("/ping.view") {
                        json!({"status":"ok","serverVersion":"1.0.0"})
                    } else if url.path().ends_with("/getOpenSubsonicExtensions.view") {
                        json!({"status":"ok","openSubsonicExtensions":[]})
                    } else {
                        assert!(
                            url.path().ends_with("/jukeboxControl.view"),
                            "Unexpected fixture request: {}",
                            url.path()
                        );
                        worker_queries.lock().unwrap().push(args.clone());
                        let mut state = worker_state.lock().unwrap();
                        match parameter("action") {
                            "get" => {}
                            "status" => {}
                            "set" => {
                                state["entry"] = args
                                    .iter()
                                    .filter(|(key, _)| key == "id")
                                    .map(|(_, id)| json!({"id":id,"title":id,"duration":20}))
                                    .collect::<Vec<_>>()
                                    .into();
                                state["currentIndex"] = 0.into();
                                state["position"] = 0.into();
                                state["playing"] = false.into();
                            }
                            "add" => state["entry"].as_array_mut().unwrap().push(
                                json!({"id":parameter("id"),"title":parameter("id"),"duration":20}),
                            ),
                            "remove" => {
                                let index = parameter("index").parse::<usize>().unwrap();
                                state["entry"].as_array_mut().unwrap().remove(index);
                                let current = state["currentIndex"].as_u64().unwrap();
                                if (index as u64) < current {
                                    state["currentIndex"] = (current - 1).into();
                                }
                            }
                            "skip" => {
                                state["currentIndex"] =
                                    parameter("index").parse::<u64>().unwrap().into();
                                state["position"] =
                                    parameter("offset").parse::<u64>().unwrap().into();
                            }
                            "setGain" => {
                                state["gain"] = parameter("gain").parse::<f64>().unwrap().into()
                            }
                            "start" => state["playing"] = true.into(),
                            "stop" => state["playing"] = false.into(),
                            "clear" => {
                                state["entry"] = json!([]);
                                state["currentIndex"] = (-1).into();
                                state["position"] = 0.into();
                            }
                            action => panic!("Unknown Jukebox action: {action}"),
                        }
                        if parameter("action") == "status" {
                            json!({"status":"ok","jukeboxStatus":state.clone()})
                        } else {
                            json!({"status":"ok","jukeboxPlaylist":state.clone()})
                        }
                    };
                    let body = json!({"subsonic-response":response}).to_string();
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                }
            });
            Self {
                api,
                state,
                queries,
                done,
                worker: Some(worker),
            }
        }
        fn actions(&self) -> Vec<String> {
            self.queries
                .lock()
                .unwrap()
                .iter()
                .filter_map(|query| {
                    query
                        .iter()
                        .find(|(key, _)| key == "action")
                        .map(|(_, action)| action.clone())
                })
                .collect()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.done.store(true, Ordering::SeqCst);
            self.worker.take().unwrap().join().unwrap();
        }
    }

    #[test]
    fn jukebox_recovers_queue_controls_duplicates_and_preserves_server_auto_advance() {
        for kind in [ServerType::Navidrome, ServerType::Subsonic] {
            let fixture = Fixture::new(kind);
            fixture.api.discover_features();
            assert!(fixture.api.features().jukebox);
            let mut engine = Engine::new(fixture.api.clone()).unwrap();
            assert_eq!(
                (
                    engine.state.index,
                    engine.state.position,
                    engine.state.playing
                ),
                (1, 7., true)
            );
            assert_eq!(engine.state.current().unwrap().name, "Second");
            assert_eq!(
                fixture.actions(),
                if kind == ServerType::Subsonic {
                    vec!["status", "get"]
                } else {
                    vec!["get"]
                }
            );
            let item = Item {
                id: "duplicate".into(),
                name: "Second".into(),
                duration: 20.,
                server_id: fixture.api.server.id,
                ..Default::default()
            };
            engine.next(&item, "entry-two".into()).unwrap();
            assert_eq!(engine.state.index, 1);
            {
                let mut state = fixture.state.lock().unwrap();
                state["currentIndex"] = 2.into();
                state["position"] = 3.into();
            }
            assert!(engine.poll().unwrap());
            let before = fixture.actions().len();
            engine
                .load(&item, "entry-two", 0., &Settings::default())
                .unwrap();
            assert_eq!((engine.state.index, engine.state.position), (0, 3.));
            assert_eq!(
                &fixture.actions()[before..],
                ["remove", "remove", "setGain", "start"]
            );
            assert_eq!(
                fixture.state.lock().unwrap()["entry"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            engine
                .load(&item, "another-duplicate", 0., &Settings::default())
                .unwrap();
            assert_eq!(engine.state.position, 0.);
            assert!(
                fixture
                    .actions()
                    .windows(2)
                    .any(|pair| pair == ["set", "skip"])
            );
            engine.seek(5.9).unwrap();
            assert_eq!(fixture.state.lock().unwrap()["position"], 5);
            engine.pause(true).unwrap();
            assert!(!engine.poll().unwrap());
            engine.pause(false).unwrap();
            engine
                .volume(&Settings {
                    volume: 37.,
                    ..Default::default()
                })
                .unwrap();
            assert!((engine.state.gain - 0.37).abs() < 0.0001);
            engine
                .volume(&Settings {
                    volume: 37.,
                    muted: true,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(engine.state.gain, 0.);
            let before = fixture.actions().len();
            assert!(engine.seek(f64::NAN).is_err());
            assert!(fixture.api.jukebox(Command::Gain(1.1)).is_err());
            assert!(
                engine
                    .load(
                        &Item {
                            server_id: uuid::Uuid::new_v4(),
                            ..item.clone()
                        },
                        "foreign",
                        0.,
                        &Settings::default()
                    )
                    .is_err()
            );
            assert_eq!(fixture.actions().len(), before);
            engine.seek(19.).unwrap();
            {
                let mut state = fixture.state.lock().unwrap();
                state["playing"] = false.into();
                state["position"] = 20.into();
            }
            assert!(engine.poll().unwrap());
            assert!(!engine.poll().unwrap());
            engine.stop().unwrap();
            assert!(engine.state.items.is_empty());
            assert_eq!(fixture.api.jukebox(Command::Get).unwrap().unwrap().index, 0);
            let imported = crate::migration::preview(
                &Settings::default(),
                br#"{"playback":{"type":"jukebox"}}"#,
            )
            .unwrap();
            assert!(imported.settings.jukebox);
        }
    }

    fn wait(player: &Player, predicate: impl Fn(&Event) -> bool) -> Event {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(event) = player.events.recv_timeout(Duration::from_millis(100)) {
                assert!(
                    !matches!(event, Event::Error(..)),
                    "Playback failed: {event:?}"
                );
                if predicate(&event) {
                    return event;
                }
            }
        }
        panic!("Jukebox event timed out");
    }

    #[test]
    fn native_playback_worker_routes_jukebox_without_opening_audio_and_keeps_duplicate_identity() {
        let fixture = Fixture::new(ServerType::Navidrome);
        let player = Player::new();
        let selected = player.send(Control::Jukebox(Some(Box::new(fixture.api.clone()))));
        let recovered = wait(
            &player,
            |event| matches!(event, Event::JukeboxSynced(id, _) if *id == selected),
        );
        assert!(
            matches!(recovered, Event::JukeboxSynced(_, state) if state.position == 7. && state.index == 1)
        );
        let item = Item {
            id: "duplicate".into(),
            name: "Duplicate".into(),
            duration: 20.,
            server_id: fixture.api.server.id,
            ..Default::default()
        };
        let load = |key: &str| Control::Load {
            url: "http://audio-must-not-open.invalid".into(),
            settings: Box::default(),
            position: 0.,
            gain: 1.,
            item: Box::new(item.clone()),
            key: key.into(),
        };
        let first = player.send(load("entry-one"));
        wait(
            &player,
            |event| matches!(event, Event::Started(id) if *id == first),
        );
        player.send(Control::NextUri {
            url: String::new(),
            item: Box::new(item.clone()),
            key: "entry-two".into(),
            gain: 1.,
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while fixture.state.lock().unwrap()["entry"]
            .as_array()
            .unwrap()
            .len()
            != 2
        {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        {
            let mut state = fixture.state.lock().unwrap();
            state["currentIndex"] = 1.into();
            state["position"] = 3.into();
        }
        wait(
            &player,
            |event| matches!(event, Event::Ended(id) if *id == first),
        );
        let second = player.send(load("entry-two"));
        assert_ne!(first, second);
        wait(
            &player,
            |event| matches!(event, Event::Started(id) if *id == second),
        );
        wait(
            &player,
            |event| matches!(event, Event::Position(id, position) if *id == second && *position == 3.),
        );
        assert_eq!(
            fixture
                .actions()
                .iter()
                .filter(|action| *action == "set")
                .count(),
            1
        );
        player.send(Control::Pause(true));
        wait(
            &player,
            |event| matches!(event, Event::Paused(id, true) if *id == second),
        );
        player.send(Control::Seek(8.9));
        wait(
            &player,
            |event| matches!(event, Event::Position(id, position) if *id == second && *position == 8.),
        );
        let stopped = player.send(Control::Stop);
        wait(
            &player,
            |event| matches!(event, Event::Stopped(id) if *id == stopped),
        );
        assert!(
            fixture.state.lock().unwrap()["entry"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        drop(player);
        // Output selection and Load may be queued before the worker sees either command.
        let player = Player::new();
        player.send(Control::Jukebox(Some(Box::new(fixture.api.clone()))));
        let generation = player.send(Control::Load {
            url: "http://audio-must-not-open.invalid".into(),
            settings: Box::new(Settings {
                jukebox: true,
                ..Default::default()
            }),
            position: 0.,
            gain: 1.,
            item: Box::new(item),
            key: "immediate".into(),
        });
        wait(
            &player,
            |event| matches!(event, Event::Started(id) if *id == generation),
        );
        drop(player);
    }
}
