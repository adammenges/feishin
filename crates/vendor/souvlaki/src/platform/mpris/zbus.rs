use std::collections::HashMap;
use std::convert::From;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zbus::{dbus_interface, ConnectionBuilder, SignalContext};
use zvariant::{ObjectPath, Value};

use crate::{
    LoopStatus, MediaControlEvent, MediaMetadata, MediaPlayback, MediaPosition, MprisMetadata,
    PlatformConfig, SeekDirection,
};

use super::Error;

/// A handle to OS media controls.
pub struct MediaControls {
    thread: Option<ServiceThreadHandle>,
    dbus_name: String,
    friendly_name: String,
}

struct ServiceThreadHandle {
    event_channel: mpsc::Sender<InternalEvent>,
    thread: JoinHandle<()>,
}

#[derive(Clone, PartialEq, Debug)]
enum InternalEvent {
    ChangeMetadata(OwnedMetadata),
    ChangePlayback(MediaPlayback),
    ChangeVolume(f64),
    ChangeShuffle(bool),
    ChangeLoopStatus(LoopStatus),
    Seeked(i64),
    Kill,
}

#[derive(Clone, Debug)]
struct ServiceState {
    metadata: OwnedMetadata,
    playback_status: MediaPlayback,
    volume: f64,
    shuffle: bool,
    loop_status: LoopStatus,
}

#[derive(Clone, PartialEq, Debug, Default)]
struct OwnedMetadata {
    pub title: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub cover_url: Option<String>,
    pub duration: Option<i64>,
    pub mpris: MprisMetadata,
}

impl From<MediaMetadata<'_>> for OwnedMetadata {
    fn from(other: MediaMetadata) -> Self {
        OwnedMetadata {
            title: other.title.map(|s| s.to_string()),
            artist: other.artist.map(|s| s.to_string()),
            album: other.album.map(|s| s.to_string()),
            cover_url: other.cover_url.map(|s| s.to_string()),
            duration: other
                .duration
                .map(|d| d.as_micros().try_into().unwrap_or(i64::MAX)),
            mpris: other.mpris.cloned().unwrap_or_default(),
        }
    }
}

impl MediaControls {
    /// Create media controls with the specified config.
    pub fn new(config: PlatformConfig) -> Result<Self, Error> {
        let PlatformConfig {
            dbus_name,
            display_name,
            ..
        } = config;

        Ok(Self {
            thread: None,
            dbus_name: dbus_name.to_string(),
            friendly_name: display_name.to_string(),
        })
    }

    /// Attach the media control events to a handler.
    pub fn attach<F>(&mut self, event_handler: F) -> Result<(), Error>
    where
        F: Fn(MediaControlEvent) + Send + 'static,
    {
        self.detach()?;

        let dbus_name = self.dbus_name.clone();
        let friendly_name = self.friendly_name.clone();
        let event_handler = Arc::new(Mutex::new(event_handler));
        let (event_channel, rx) = mpsc::channel();

        self.thread = Some(ServiceThreadHandle {
            event_channel,
            thread: thread::spawn(move || {
                pollster::block_on(run_service(dbus_name, friendly_name, event_handler, rx))
                    .unwrap();
            }),
        });
        Ok(())
    }
    /// Detach the event handler.
    pub fn detach(&mut self) -> Result<(), Error> {
        if let Some(ServiceThreadHandle {
            event_channel,
            thread,
        }) = self.thread.take()
        {
            event_channel.send(InternalEvent::Kill).ok();
            thread.join().map_err(|_| Error::ThreadPanicked)?;
        }
        Ok(())
    }

    /// Set the current playback status.
    pub fn set_playback(&mut self, playback: MediaPlayback) -> Result<(), Error> {
        self.send_internal_event(InternalEvent::ChangePlayback(playback))?;
        Ok(())
    }

    /// Set the metadata of the currently playing media item.
    pub fn set_metadata(&mut self, metadata: MediaMetadata) -> Result<(), Error> {
        self.send_internal_event(InternalEvent::ChangeMetadata(metadata.into()))?;
        Ok(())
    }

    /// Set the volume level (0.0 - 1.0) (Only available on MPRIS)
    pub fn set_volume(&mut self, volume: f64) -> Result<(), Error> {
        self.send_internal_event(InternalEvent::ChangeVolume(volume))?;
        Ok(())
    }

    /// Publish shuffle state after the application accepts a change.
    pub fn set_shuffle(&mut self, enabled: bool) -> Result<(), Error> {
        self.send_internal_event(InternalEvent::ChangeShuffle(enabled))
    }

    /// Publish repeat state after the application accepts a change.
    pub fn set_loop_status(&mut self, status: LoopStatus) -> Result<(), Error> {
        self.send_internal_event(InternalEvent::ChangeLoopStatus(status))
    }

    /// Emit a discontinuous position change, including seeks initiated in the app.
    pub fn notify_seeked(&mut self, position: MediaPosition) -> Result<(), Error> {
        self.send_internal_event(InternalEvent::Seeked(
            position.0.as_micros().try_into().unwrap_or(i64::MAX),
        ))
    }

    fn send_internal_event(&mut self, event: InternalEvent) -> Result<(), Error> {
        let channel = &self
            .thread
            .as_ref()
            .ok_or(Error::ThreadNotRunning)?
            .event_channel;
        channel.send(event).map_err(|_| Error::ThreadPanicked)
    }
}

struct AppInterface {
    friendly_name: String,
    event_handler: Arc<Mutex<dyn Fn(MediaControlEvent) + Send + 'static>>,
}

#[dbus_interface(name = "org.mpris.MediaPlayer2")]
impl AppInterface {
    fn raise(&self) {
        self.send_event(MediaControlEvent::Raise);
    }
    fn quit(&self) {
        self.send_event(MediaControlEvent::Quit);
    }

    #[dbus_interface(property)]
    fn can_quit(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    fn can_raise(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    fn has_tracklist(&self) -> bool {
        false
    }

    #[dbus_interface(property)]
    fn identity(&self) -> &str {
        &self.friendly_name
    }

    #[dbus_interface(property)]
    fn supported_uri_schemes(&self) -> &[&str] {
        &[]
    }

    #[dbus_interface(property)]
    fn supported_mime_types(&self) -> &[&str] {
        &[]
    }
}

impl AppInterface {
    fn send_event(&self, event: MediaControlEvent) {
        (self.event_handler.lock().unwrap())(event);
    }
}

struct PlayerInterface {
    state: ServiceState,
    event_handler: Arc<Mutex<dyn Fn(MediaControlEvent) + Send + 'static>>,
}

impl PlayerInterface {
    fn send_event(&self, event: MediaControlEvent) {
        (self.event_handler.lock().unwrap())(event);
    }
}

#[dbus_interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerInterface {
    fn next(&self) {
        self.send_event(MediaControlEvent::Next);
    }
    fn previous(&self) {
        self.send_event(MediaControlEvent::Previous);
    }
    fn pause(&self) {
        self.send_event(MediaControlEvent::Pause);
    }
    fn play_pause(&self) {
        self.send_event(MediaControlEvent::Toggle);
    }
    fn stop(&self) {
        self.send_event(MediaControlEvent::Stop);
    }
    fn play(&self) {
        self.send_event(MediaControlEvent::Play);
    }

    fn seek(&self, offset: i64) {
        let abs_offset = offset.unsigned_abs();
        let direction = if offset > 0 {
            SeekDirection::Forward
        } else {
            SeekDirection::Backward
        };

        self.send_event(MediaControlEvent::SeekBy(
            direction,
            Duration::from_micros(abs_offset),
        ));
    }

    fn set_position(&self, track_id: zvariant::ObjectPath, position: i64) {
        if track_id.as_str() != self.state.metadata.mpris.track_id
            || track_id.as_str() == "/org/mpris/MediaPlayer2/TrackList/NoTrack"
        {
            return;
        }
        if let Ok(micros) = position.try_into() {
            if let Some(duration) = self.state.metadata.duration {
                // If the Position argument is greater than the track length, do nothing.
                if position > duration {
                    return;
                }
            }

            let position = Duration::from_micros(micros);
            self.send_event(MediaControlEvent::SetPosition(MediaPosition(position)));
        }
    }

    fn open_uri(&self, uri: String) {
        // NOTE: we should check if the URI is in the `SupportedUriSchemes` list.
        self.send_event(MediaControlEvent::OpenUri(uri));
    }

    #[dbus_interface(property)]
    fn playback_status(&self) -> &'static str {
        match self.state.playback_status {
            MediaPlayback::Playing { .. } => "Playing",
            MediaPlayback::Paused { .. } => "Paused",
            MediaPlayback::Stopped => "Stopped",
        }
    }

    #[dbus_interface(property)]
    fn rate(&self) -> f64 {
        1.0
    }

    #[dbus_interface(property)]
    fn shuffle(&self) -> bool {
        self.state.shuffle
    }

    #[dbus_interface(property)]
    fn set_shuffle(&self, enabled: bool) {
        self.send_event(MediaControlEvent::SetShuffle(enabled));
    }

    #[dbus_interface(property)]
    fn loop_status(&self) -> &str {
        self.state.loop_status.as_str()
    }

    #[dbus_interface(property)]
    fn set_loop_status(&self, status: &str) -> zbus::Result<()> {
        let status = match status {
            "None" => LoopStatus::None,
            "Track" => LoopStatus::Track,
            "Playlist" => LoopStatus::Playlist,
            _ => return Err(zbus::fdo::Error::InvalidArgs("Invalid LoopStatus".into()).into()),
        };
        self.send_event(MediaControlEvent::SetLoopStatus(status));
        Ok(())
    }

    #[dbus_interface(signal)]
    async fn seeked(ctxt: &SignalContext<'_>, position: i64) -> zbus::Result<()>;

    #[dbus_interface(property)]
    fn metadata(&self) -> HashMap<&str, Value<'_>> {
        let mut dict = HashMap::<&str, Value>::new();
        let metadata = &self.state.metadata;
        let extra = &metadata.mpris;
        if let Ok(id) = ObjectPath::try_from(extra.track_id.as_str()) {
            dict.insert("mpris:trackid", Value::new(id));
        }
        if let Some(length) = metadata.duration {
            dict.insert("mpris:length", Value::new(length));
        }
        for (key, value) in [
            ("mpris:artUrl", &metadata.cover_url),
            ("xesam:title", &metadata.title),
            ("xesam:album", &metadata.album),
            ("xesam:asText", &extra.lyrics),
            ("xesam:contentCreated", &extra.content_created),
            ("xesam:lastUsed", &extra.last_used),
        ] {
            if let Some(value) = value {
                dict.insert(key, Value::new(value.as_str()));
            }
        }
        for (key, value) in [
            ("xesam:artist", &extra.artists),
            ("xesam:albumArtist", &extra.album_artists),
            ("xesam:genre", &extra.genres),
        ] {
            if !value.is_empty() {
                dict.insert(key, Value::new(value.clone()));
            }
        }
        if extra.artists.is_empty() {
            if let Some(artist) = &metadata.artist {
                dict.insert("xesam:artist", Value::new(vec![artist.as_str()]));
            }
        }
        if let Some(comment) = &extra.comment {
            dict.insert("xesam:comment", Value::new(vec![comment.as_str()]));
        }
        for (key, value) in [
            ("xesam:audioBPM", extra.audio_bpm),
            ("xesam:discNumber", extra.disc_number),
            ("xesam:trackNumber", extra.track_number),
            ("xesam:useCount", extra.use_count),
        ] {
            if let Some(value) = value {
                dict.insert(key, Value::new(value));
            }
        }
        if let Some(value) = extra.user_rating {
            dict.insert("xesam:userRating", Value::new(value));
        }
        dict
    }

    #[dbus_interface(property)]
    fn volume(&self) -> f64 {
        self.state.volume
    }

    #[dbus_interface(property)]
    fn set_volume(&self, volume: f64) {
        self.send_event(MediaControlEvent::SetVolume(volume));
    }

    #[dbus_interface(property)]
    fn position(&self) -> i64 {
        let position = match self.state.playback_status {
            MediaPlayback::Playing {
                progress: Some(pos),
            }
            | MediaPlayback::Paused {
                progress: Some(pos),
            } => pos.0.as_micros(),
            _ => 0,
        };

        position.try_into().unwrap_or(0)
    }

    #[dbus_interface(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[dbus_interface(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[dbus_interface(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    fn can_seek(&self) -> bool {
        true
    }

    #[dbus_interface(property)]
    fn can_control(&self) -> bool {
        true
    }
}

async fn run_service(
    dbus_name: String,
    friendly_name: String,
    event_handler: Arc<Mutex<dyn Fn(MediaControlEvent) + Send + 'static>>,
    event_channel: mpsc::Receiver<InternalEvent>,
) -> zbus::Result<()> {
    let app = AppInterface {
        friendly_name,
        event_handler: event_handler.clone(),
    };

    let player = PlayerInterface {
        state: ServiceState {
            metadata: OwnedMetadata::default(),
            playback_status: MediaPlayback::Stopped,
            volume: 1.0,
            shuffle: false,
            loop_status: LoopStatus::None,
        },
        event_handler,
    };

    let name = format!("org.mpris.MediaPlayer2.{dbus_name}");
    let path = ObjectPath::try_from("/org/mpris/MediaPlayer2")?;
    let connection = ConnectionBuilder::session()?
        .serve_at(&path, app)?
        .serve_at(&path, player)?
        .name(name.as_str())?
        .build()
        .await?;

    loop {
        if let Ok(event) = event_channel.recv_timeout(Duration::from_millis(10)) {
            if event == InternalEvent::Kill {
                break;
            }

            let interface_ref = connection
                .object_server()
                .interface::<_, PlayerInterface>(&path)
                .await?;
            let mut interface = interface_ref.get_mut().await;
            let ctxt = SignalContext::new(&connection, &path)?;

            match event {
                InternalEvent::ChangeMetadata(metadata) => {
                    if interface.state.metadata != metadata {
                        interface.state.metadata = metadata;
                        interface.metadata_changed(&ctxt).await?;
                    }
                }
                InternalEvent::ChangePlayback(playback) => {
                    let status_changed = std::mem::discriminant(&interface.state.playback_status)
                        != std::mem::discriminant(&playback);
                    interface.state.playback_status = playback;
                    if status_changed {
                        interface.playback_status_changed(&ctxt).await?;
                    }
                }
                InternalEvent::ChangeVolume(volume) => {
                    if interface.state.volume != volume {
                        interface.state.volume = volume;
                        interface.volume_changed(&ctxt).await?;
                    }
                }
                InternalEvent::ChangeShuffle(enabled) => {
                    if interface.state.shuffle != enabled {
                        interface.state.shuffle = enabled;
                        interface.shuffle_changed(&ctxt).await?;
                    }
                }
                InternalEvent::ChangeLoopStatus(status) => {
                    if interface.state.loop_status != status {
                        interface.state.loop_status = status;
                        interface.loop_status_changed(&ctxt).await?;
                    }
                }
                InternalEvent::Seeked(position) => PlayerInterface::seeked(&ctxt, position).await?,
                InternalEvent::Kill => (),
            }
        }
    }

    Ok(())
}
