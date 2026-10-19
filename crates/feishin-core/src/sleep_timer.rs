use crate::model::Item;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum Mode {
    Timed(Duration),
    Song(Uuid),
    Album(Uuid, String),
}
#[derive(Clone, Debug)]
pub struct Timer {
    pub mode: Mode,
    tick: Instant,
}
fn album(item: &Item) -> String {
    if item.album_id.is_empty() {
        format!("{}\0{}", item.album, item.album_artist)
    } else {
        item.album_id.clone()
    }
}
impl Timer {
    pub fn timed(duration: Duration, now: Instant) -> Self {
        Self {
            mode: Mode::Timed(duration),
            tick: now,
        }
    }
    pub fn song(key: Uuid, now: Instant) -> Self {
        Self {
            mode: Mode::Song(key),
            tick: now,
        }
    }
    pub fn album(item: &Item, now: Instant) -> Self {
        Self {
            mode: Mode::Album(item.server_id, album(item)),
            tick: now,
        }
    }
    pub fn tick(&mut self, now: Instant, playing: bool) -> bool {
        let elapsed = now.saturating_duration_since(self.tick);
        self.tick = now;
        if let Mode::Timed(remaining) = &mut self.mode {
            if playing {
                *remaining = remaining.saturating_sub(elapsed);
            }
            remaining.is_zero()
        } else {
            false
        }
    }
    pub fn stop_at_end(&self, key: Uuid, current: &Item, next: Option<&Item>) -> bool {
        match &self.mode {
            Mode::Song(target) => *target == key,
            Mode::Album(server, target) => {
                *server == current.server_id
                    && *target == album(current)
                    && next.is_none_or(|next| next.server_id != *server || album(next) != *target)
            }
            Mode::Timed(_) => false,
        }
    }
    /// Cancels an end timer when the user leaves its target. Only track mode pauses.
    pub fn track_changed(&self, key: Uuid, item: &Item) -> Option<bool> {
        match &self.mode {
            Mode::Song(target) if *target != key => Some(true),
            Mode::Album(server, target) if *server != item.server_id || *target != album(item) => {
                Some(false)
            }
            _ => None,
        }
    }
    pub fn label(&self) -> String {
        match &self.mode {
            Mode::Timed(remaining) if remaining.as_secs() >= 3600 => format!(
                "{}:{:02}:{:02}",
                remaining.as_secs() / 3600,
                remaining.as_secs() / 60 % 60,
                remaining.as_secs() % 60
            ),
            Mode::Timed(remaining) => format!(
                "{}:{:02}",
                remaining.as_secs() / 60,
                remaining.as_secs() % 60
            ),
            Mode::Song(_) => "Sleep at end of track".into(),
            Mode::Album(_, _) => "Sleep at end of album".into(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timed_sleep_counts_playback_only_and_end_modes_respect_track_album_and_server() {
        let now = Instant::now();
        let mut timed = Timer::timed(Duration::from_secs(10), now);
        assert!(!timed.tick(now + Duration::from_secs(4), true));
        assert!(!timed.tick(now + Duration::from_secs(100), false));
        assert!(!timed.tick(now + Duration::from_secs(105), true));
        assert!(timed.tick(now + Duration::from_secs(106), true));
        let key = Uuid::new_v4();
        let next_key = Uuid::new_v4();
        let current = Item {
            album_id: "album".into(),
            server_id: Uuid::new_v4(),
            ..Default::default()
        };
        let next = Item {
            album_id: "other".into(),
            ..current.clone()
        };
        let song = Timer::song(key, now);
        assert!(song.stop_at_end(key, &current, Some(&current)));
        assert_eq!(song.track_changed(next_key, &current), Some(true));
        let album = Timer::album(&current, now);
        assert!(!album.stop_at_end(key, &current, Some(&current)));
        assert!(album.stop_at_end(key, &current, Some(&next)));
        assert!(album.stop_at_end(key, &current, None));
        assert_eq!(album.track_changed(next_key, &current), None);
        assert_eq!(album.track_changed(next_key, &next), Some(false));
        let other_server = Item {
            server_id: Uuid::new_v4(),
            ..current.clone()
        };
        assert!(album.stop_at_end(key, &current, Some(&other_server)));
    }
}
