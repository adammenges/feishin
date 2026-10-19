use crate::app::Feishin;
use gpui::Context;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Default)]
pub struct TrackNotifications {
    last: Option<Uuid>,
    pending: Option<(Uuid, Instant)>,
}
impl TrackNotifications {
    pub fn arm(&mut self, key: Option<Uuid>, enabled: bool, now: Instant) {
        if self.last == key {
            return;
        }
        self.last = key;
        self.pending = key
            .filter(|_| enabled)
            .map(|key| (key, now + Duration::from_secs(1)));
    }
    pub fn take_due(&mut self, key: Option<Uuid>, enabled: bool, now: Instant) -> bool {
        let Some((pending, due)) = self.pending else {
            return false;
        };
        if !enabled || key != Some(pending) {
            self.pending = None;
            return false;
        }
        if now < due {
            return false;
        }
        self.pending = None;
        true
    }
}
impl Feishin {
    pub fn poll_track_notifications(&mut self, cx: &mut Context<Self>) {
        if !self.notifications.take_due(
            self.state.queue.current,
            self.state.settings.notify_tracks && self.radio_item.is_none(),
            Instant::now(),
        ) {
            return;
        }
        let Some(entry) = self.state.queue.current() else {
            return;
        };
        let mut notice = notify_rust::Notification::new();
        notice
            .appname("Feishin")
            .summary(&entry.item.name)
            .body(&format!("{}\n{}", entry.item.artist, entry.item.album));
        #[cfg(target_os = "linux")]
        {
            notice.body(&format!(
                "{}\n{}",
                markup(&entry.item.artist),
                markup(&entry.item.album)
            ));
            notice.hint(notify_rust::Hint::SuppressSound(true));
        }
        if let Some(path) = self
            .artwork_path_at(&entry.item, feishin_core::images::Placement::Sidebar)
            .and_then(|path| path.to_str())
        {
            notice.image_path(path);
        }
        // Headless verification must never deliver operating-system notifications.
        #[cfg(not(test))]
        cx.background_executor()
            .spawn(async move {
                #[cfg(target_os = "macos")]
                if let Err(error) = notify_rust::set_application("org.jeffvli.feishin.rust") {
                    log::debug!("Notification application registration failed: {error}");
                }
                if let Err(error) = notice.show() {
                    log::warn!("Track notification failed: {error}");
                }
            })
            .detach();
        #[cfg(test)]
        let _ = (notice, cx);
    }
}
#[cfg(target_os = "linux")]
fn markup(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_track_notices_deduplicate_resume_and_cancel_superseded_tracks() {
        let mut notices = TrackNotifications::default();
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let now = Instant::now();
        notices.arm(Some(a), true, now);
        assert!(!notices.take_due(Some(a), true, now));
        notices.arm(Some(b), true, now + Duration::from_millis(500));
        assert!(!notices.take_due(Some(b), true, now + Duration::from_secs(1)));
        assert!(notices.take_due(Some(b), true, now + Duration::from_secs(2)));
        notices.arm(Some(b), true, now + Duration::from_secs(3));
        assert!(!notices.take_due(Some(b), true, now + Duration::from_secs(5)));
        notices.arm(Some(a), true, now);
        assert!(!notices.take_due(Some(a), false, now + Duration::from_secs(2)));
        assert!(!notices.take_due(Some(a), true, now + Duration::from_secs(3)));
        notices.arm(Some(b), true, now);
        assert!(!notices.take_due(None, true, now + Duration::from_secs(2)));
    }
}
