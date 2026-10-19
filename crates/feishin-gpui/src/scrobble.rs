use crate::app::{Feishin, Route};
use feishin_core::{
    model::{Item, ItemKind, ServerType, duration_label},
    storage::ScrobbleMode,
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, Sizable, StyledExt, button::ButtonVariants};

#[derive(Default)]
pub(crate) struct Listening {
    pub seconds: f64,
    pub submitted: bool,
    sample: Option<f64>,
    reported_position: f64,
}
impl Listening {
    pub fn reset(&mut self, position: f64) {
        self.seconds = 0.;
        self.submitted = false;
        self.sample = Some(position);
        self.reported_position = 0.;
    }
    pub fn progress_due(&mut self, position: f64) -> bool {
        if position - self.reported_position < 10. {
            return false;
        }
        self.reported_position = position;
        true
    }
    pub fn seek(&mut self, position: f64) {
        if position < 5. && self.sample.is_none_or(|previous| previous >= 5.) {
            self.reset(position);
        }
        self.sample = Some(position);
    }
    /// Returns whether this sample can trigger automatic submission.
    pub fn observe(&mut self, previous: f64, position: f64, playing: bool) -> bool {
        if !playing {
            self.sample = Some(position);
            return false;
        }
        if position < previous && position < 5. && previous >= 10. {
            *self = Self::default();
            return false;
        }
        let last = self.sample.unwrap_or(previous);
        let delta = position - last;
        if self.sample.is_some() && position < last && position < 5. && last >= 5. {
            self.reset(position);
        } else if delta > 0. && delta <= 5. {
            self.seconds += delta;
        }
        self.sample = Some(position);
        true
    }
}

#[derive(Default)]
pub(crate) struct TrackReports {
    current: Option<feishin_core::queue::Entry>,
    position: f64,
    pending: Option<(
        std::time::Instant,
        Option<(feishin_core::queue::Entry, f64)>,
    )>,
    stopped: bool,
}
impl TrackReports {
    pub fn change(
        &mut self,
        entry: Option<feishin_core::queue::Entry>,
        position: f64,
        now: std::time::Instant,
    ) -> bool {
        let changed =
            self.current.as_ref().map(|entry| entry.key) != entry.as_ref().map(|entry| entry.key);
        if changed {
            let previous = (!self.stopped)
                .then(|| self.current.clone().map(|entry| (entry, self.position)))
                .flatten();
            self.pending = Some((now + std::time::Duration::from_secs(2), previous));
        }
        self.current = entry;
        self.position = position;
        self.stopped = false;
        changed
    }
    pub fn position(&mut self, position: f64) {
        self.position = position;
    }
    pub fn stop(&mut self) {
        self.pending = None;
        self.stopped = true;
    }
    fn take_due(
        &mut self,
        now: std::time::Instant,
    ) -> Option<Option<(feishin_core::queue::Entry, f64)>> {
        if self.pending.as_ref().is_some_and(|(due, _)| now >= *due) {
            self.pending.take().map(|(_, previous)| previous)
        } else {
            None
        }
    }
}

struct StatusPanel {
    app: WeakEntity<Feishin>,
    _subscription: Option<Subscription>,
}
impl Render for StatusPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.app
            .update(cx, |app, cx| app.scrobble_panel(cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}
#[derive(IntoElement)]
struct ConditionBar {
    index: usize,
    progress: f32,
    rtl: bool,
}
impl RenderOnce for ConditionBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = window.use_keyed_state(("scrobble-condition", self.index), cx, |_, _| {
            gpui_component::animation::CssTransition::new(
                self.progress,
                std::time::Duration::from_millis(100),
                [0.25, 0.1, 0.25, 1.],
            )
        });
        let (progress, animating) = motion.update(cx, |motion, cx| {
            let now = cx.background_executor().now();
            motion.retarget(self.progress, now);
            motion.value(now)
        });
        if animating {
            window.request_animation_frame();
        }
        div()
            .h(rems(3. / 16.))
            .w_full()
            .flex()
            .rounded_full()
            .overflow_hidden()
            .when(self.rtl, |bar| bar.flex_row_reverse())
            .bg(cx.theme().muted)
            .child(
                div()
                    .id(("scrobble-condition-fill", self.index))
                    .debug_selector(move || format!("scrobble-condition-fill-{}", self.index))
                    .h_full()
                    .w(relative(progress / 100.))
                    .rounded_full()
                    .bg(cx.theme().primary),
            )
    }
}
impl Feishin {
    pub(crate) fn begin_track_reports(&mut self, cx: &Context<Self>) {
        let entry = self
            .current_entry()
            .filter(|entry| entry.item.kind == ItemKind::Song)
            .cloned();
        let position = self.current_position();
        if self
            .track_reports
            .change(entry, position, cx.background_executor().now())
            || position < self.last_position
        {
            self.listening = Default::default();
        }
        self.last_position = position;
    }
    pub(crate) fn poll_track_reports(&mut self, cx: &mut Context<Self>) {
        if !self.state.settings.scrobble || self.state.settings.private_mode {
            self.track_reports.pending = None;
            return;
        }
        let Some(previous) = self.track_reports.take_due(cx.background_executor().now()) else {
            return;
        };
        if let Some((previous, position)) = previous
            && self
                .sessions
                .get(&previous.item.server_id)
                .is_some_and(|api| {
                    api.server.kind != ServerType::Jellyfin || previous.item.resumable
                })
        {
            self.dispatch_report(previous, "stop", position, cx);
        }
        if self.playing
            && self.loaded
            && self.current_entry().is_some_and(|entry| {
                self.track_reports
                    .current
                    .as_ref()
                    .is_some_and(|reported| reported.key == entry.key)
            })
        {
            self.report("start", cx);
        }
    }
    pub(crate) fn scrobble_active(&self) -> bool {
        self.state.settings.scrobble
            && !self.state.settings.private_mode
            && self
                .current_entry()
                .is_some_and(|entry| entry.item.kind == ItemKind::Song && !entry.item.id.is_empty())
    }
    pub(crate) fn scrobble_status(&self, cx: &mut Context<Self>) -> AnyElement {
        let app = cx.weak_entity();
        let time = duration_label(self.current_position());
        div()
            .id("scrobble-status")
            .debug_selector(|| "scrobble-status".into())
            .h_flex()
            .items_center()
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            .gap_2()
            .font_semibold()
            .aria_label(format!("{}, {time}", self.tr("setting.scrobble")))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .size(px(6. * self.state.settings.zoom))
                    .rounded_full()
                    .bg(if self.listening.submitted {
                        cx.theme().primary
                    } else {
                        gpui::transparent_black()
                    }),
            )
            .child(time)
            .hoverable_tooltip(move |_, cx| {
                cx.new(|cx| StatusPanel {
                    _subscription: app
                        .upgrade()
                        .map(|parent| cx.observe(&parent, |_, _, cx| cx.notify())),
                    app: app.clone(),
                })
                .into()
            })
            .into_any_element()
    }
    pub(crate) fn scrobble_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.state.settings.scrobble && !self.state.settings.private_mode;
        let active = self.scrobble_active();
        div()
            .id("scrobble-panel")
            .debug_selector(|| "scrobble-panel".into())
            .v_flex()
            .w(px(280. * self.state.settings.zoom))
            .p_4()
            .gap_3()
            .rounded_md()
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .border_1()
            .border_color(cx.theme().border)
            .on_click(|_, _, cx| cx.stop_propagation())
            .when(!enabled, |panel| {
                panel.child(self.tr("form.privateMode.enabled"))
            })
            .when(enabled, |panel| {
                let duration = self.current_entry().map_or(0., |entry| entry.item.duration);
                panel
                    .children(
                        conditions(&self.listening, duration, &self.state.settings)
                            .into_iter()
                            .enumerate()
                            .map(|(index, (label, progress))| {
                                div()
                                    .v_flex()
                                    .gap_1()
                                    .text_xs()
                                    .child(label)
                                    .child(ConditionBar {
                                        index,
                                        progress: progress as f32,
                                        rtl: crate::appearance::rtl(cx),
                                    })
                            }),
                    )
                    .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .child(
                                crate::styles::button("scrobble-reset", cx)
                                    .debug_selector(|| "scrobble-reset".into())
                                    .small()
                                    .outline()
                                    .flex_1()
                                    .label(self.tr("common.reset"))
                                    .disabled(!active)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        if view.scrobble_active() {
                                            view.listening.reset(view.current_position());
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                crate::styles::button("scrobble-submit", cx)
                                    .debug_selector(|| "scrobble-submit".into())
                                    .small()
                                    .primary()
                                    .flex_1()
                                    .label(self.tr("player.scrobbleForceSubmit"))
                                    .disabled(!active || self.listening.submitted)
                                    .on_click(
                                        cx.listener(|view, _, _, cx| view.submit_scrobble(cx)),
                                    ),
                            ),
                    )
            })
            .into_any_element()
    }
    pub(crate) fn submit_scrobble(&mut self, cx: &mut Context<Self>) {
        if !self.scrobble_active() || self.listening.submitted {
            return;
        }
        self.listening.submitted = true;
        self.report("submit", cx);
        cx.notify();
    }
    pub fn report(&mut self, event: &'static str, cx: &mut Context<Self>) {
        if event == "stop" {
            self.track_reports.stop();
        }
        if !self.scrobble_active() || (event == "start" && self.track_reports.pending.is_some()) {
            return;
        }
        let entry = self.current_entry().unwrap().clone();
        let event = if event == "seek" {
            let now = cx.background_executor().now();
            if self
                .last_seek_report
                .is_some_and(|last| now.duration_since(last) < std::time::Duration::from_secs(1))
            {
                return;
            }
            self.last_seek_report = Some(now);
            self.listening.reported_position = self.current_position();
            if !self.loaded {
                return;
            }
            if self.playing { "unpause" } else { "pause" }
        } else {
            event
        };
        let position = if event == "submit" {
            entry.item.duration
        } else {
            self.current_position()
        };
        self.dispatch_report(entry, event, position, cx);
    }
    fn dispatch_report(
        &mut self,
        entry: feishin_core::queue::Entry,
        event: &'static str,
        position: f64,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.sessions.get(&entry.item.server_id).cloned() else {
            return;
        };
        let speed = if self.state.settings.jukebox {
            1.
        } else {
            self.state.settings.speed
        };
        let item = entry.item.clone();
        let queue = queue_window(&self.state.queue);
        let previous = self.report_task.take();
        self.report_task = Some(cx.spawn(async move |view, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            if !view
                .read_with(cx, |view, _| {
                    view.state.settings.scrobble && !view.state.settings.private_mode
                })
                .unwrap_or(false)
            {
                return;
            }
            let result =
                smol::unblock(move || api.scrobble(&item, event, position, speed, &queue)).await;
            view.update(cx, |view, cx| {
                if let Err(error) = result {
                    log::warn!("Playback report failed: {error}");
                    return;
                }
                if event == "submit" {
                    view.scrobble_succeeded(&entry.item, cx);
                    if view
                        .current_entry()
                        .is_some_and(|current| current.key == entry.key)
                        && view.playing
                        && view
                            .sessions
                            .get(&entry.item.server_id)
                            .is_some_and(|api| api.server.kind == ServerType::Jellyfin)
                    {
                        view.report("unpause", cx);
                    }
                }
            })
            .ok();
        }));
    }
    pub(crate) fn scrobble_succeeded(&mut self, item: &Item, cx: &mut Context<Self>) {
        for entry in &mut self.state.queue.entries {
            if entry.item.server_id == item.server_id && entry.item.id == item.id {
                entry.item.play_count = entry.item.play_count.saturating_add(1);
            }
        }
        if self.state.active_server == Some(item.server_id) {
            for kind in [
                feishin_core::home::Kind::RecentlyPlayed,
                feishin_core::home::Kind::MostPlayed,
            ] {
                if self.route == Route::Home
                    && self
                        .state
                        .settings
                        .home
                        .enabled()
                        .any(|enabled| enabled == kind)
                {
                    self.load_home_section(kind, false, cx);
                } else {
                    self.home_sections.remove(&kind);
                }
            }
            for section in ["topSongs", "favoriteSongs"] {
                if self.artist_overview_active() {
                    self.load_artist_section(section, cx);
                } else {
                    self.artist_sections.remove(section);
                }
            }
            self.album_metadata_cache.remove(&item.album_id);
            if matches!(&self.route,Route::Detail(album) if album.kind==ItemKind::Album && album.id==item.album_id && album.server_id==item.server_id)
            {
                self.load_detail_metadata(cx);
            }
        }
        self.save();
        cx.notify();
    }
    pub(crate) fn commit_seek(&mut self, position: f64, cx: &mut Context<Self>) {
        if !position.is_finite() || position < 0. {
            return;
        }
        self.seek_preview.clear();
        if self.scrobble_active() {
            self.listening.seek(position);
        }
        self.player
            .send(feishin_core::playback::Control::Seek(position));
        self.state.queue.position = position;
        self.track_reports.position(position);
        self.lyric_position_at = std::time::Instant::now();
        self.report("seek", cx);
        #[cfg(target_os = "linux")]
        if let Some(media) = &mut self.media
            && let Ok(duration) = std::time::Duration::try_from_secs_f64(position)
        {
            let _ = media.notify_seeked(souvlaki::MediaPosition(duration));
        }
        cx.notify();
    }
}
fn queue_window(queue: &feishin_core::queue::Queue) -> Vec<String> {
    let entries = queue.ordered();
    let Some(index) = entries
        .iter()
        .position(|entry| Some(entry.key) == queue.current)
    else {
        return Vec::new();
    };
    let window = entries
        .iter()
        .skip(index)
        .take(20)
        .map(|entry| entry.item.id.clone())
        .collect::<Vec<_>>();
    if window.len() > 1 { window } else { Vec::new() }
}

fn conditions(
    listening: &Listening,
    duration: f64,
    settings: &feishin_core::storage::Settings,
) -> Vec<(String, f64)> {
    let mut result = Vec::new();
    for (show, value, limit, suffix) in [
        (
            settings.scrobble_mode != ScrobbleMode::Percentage,
            listening.seconds,
            f64::from(settings.scrobble_seconds),
            "s",
        ),
        (
            settings.scrobble_mode != ScrobbleMode::Seconds,
            if duration > 0. {
                listening.seconds / duration * 100.
            } else {
                0.
            },
            f64::from(settings.scrobble_percentage),
            "%",
        ),
    ] {
        if show {
            let displayed = if listening.submitted {
                limit
            } else if limit > 0. {
                value.min(limit)
            } else {
                value
            };
            result.push((
                format!("{displayed:.1}{suffix} / {limit}{suffix}"),
                if limit > 0. {
                    (displayed / limit * 100.).clamp(0., 100.)
                } else {
                    0.
                },
            ));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    fn fixture_request(listener: &std::net::TcpListener) -> (std::net::TcpStream, String) {
        use std::{
            io::Read,
            thread,
            time::{Duration, Instant},
        };
        let started = Instant::now();
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        started.elapsed() < Duration::from_secs(8),
                        "Playback report missing"
                    );
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("{error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut bytes = [0; 4096];
        loop {
            let size = stream.read(&mut bytes).unwrap();
            assert!(size > 0);
            request.extend_from_slice(&bytes[..size]);
            if let Some(end) = request.windows(4).position(|slice| slice == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':')
                            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                            .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        (stream, String::from_utf8(request).unwrap())
    }

    #[test]
    fn delayed_track_reports_match_original_changes_and_stopped_resumes() {
        use std::time::{Duration, Instant};
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/scrobble-lifecycle-reference.json"))
                .unwrap();
        for case in cases {
            let start = Instant::now();
            let mut reports = TrackReports::default();
            let mut actual = Vec::new();
            let mut status = "PLAYING";
            let jellyfin = case["server"] == "jellyfin";
            let resumable = case["resumable"] == true;
            for frame in case["frames"].as_array().unwrap() {
                let at = frame["at"].as_u64().unwrap();
                let now = start + Duration::from_millis(at);
                if let Some(previous) = reports.take_due(now) {
                    if let Some((previous, position)) = previous
                        && (!jellyfin || resumable)
                    {
                        actual.push((at, "stop".to_owned(), previous.item.id, position));
                    }
                    if status == "PLAYING"
                        && let Some(current) = &reports.current
                    {
                        actual.push((
                            at,
                            "start".into(),
                            current.item.id.clone(),
                            reports.position,
                        ));
                    }
                }
                match frame["event"].as_str().unwrap() {
                    "change" => {
                        reports.change(
                            Some(feishin_core::queue::Entry {
                                key: uuid::Uuid::from_u128(u128::from(at) + 1),
                                item: Item {
                                    id: frame["song"].as_str().unwrap().into(),
                                    duration: 100.,
                                    resumable,
                                    ..Default::default()
                                },
                            }),
                            0.,
                            now,
                        );
                    }
                    "seek" | "poll" => reports.position(frame["timestamp"].as_f64().unwrap()),
                    "stop" => {
                        reports.stop();
                        if jellyfin {
                            actual.push((
                                at,
                                "stop".into(),
                                reports.current.as_ref().unwrap().item.id.clone(),
                                reports.position,
                            ));
                        }
                    }
                    "play" if status == "STOPPED" => {
                        reports.change(reports.current.clone(), reports.position, now);
                        if jellyfin {
                            actual.push((
                                at,
                                "start".into(),
                                reports.current.as_ref().unwrap().item.id.clone(),
                                reports.position,
                            ));
                        }
                    }
                    _ => {}
                }
                status = frame["status"].as_str().unwrap();
            }
            let expected = case["reports"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|report| matches!(report["event"].as_str(), Some("start" | "stop")))
                .map(|report| {
                    (
                        report["at"].as_u64().unwrap() - 100000,
                        report["event"].as_str().unwrap().to_owned(),
                        report["id"].as_str().unwrap().to_owned(),
                        report["position"].as_f64().unwrap() / if jellyfin { 1e7 } else { 1000. },
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{} resumable={resumable}", case["server"]);
            reports.change(None, 0., start + Duration::from_secs(10));
            reports.stop();
            assert!(reports.take_due(start + Duration::from_secs(20)).is_none());
        }
    }

    #[test]
    fn listening_and_status_conditions_match_original_hooks() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/scrobble-status-reference.json")).unwrap();
        for case in reference["accounting"].as_array().unwrap() {
            let active = case["enabled"] == true && case["privateMode"] == false;
            let mut listening = Listening::default();
            if active {
                listening.reset(0.);
            }
            let settings = feishin_core::storage::Settings {
                scrobble: true,
                scrobble_seconds: 60.,
                scrobble_percentage: 50.,
                ..Default::default()
            };
            for frame in case["frames"].as_array().unwrap() {
                let time = frame["time"].as_f64().unwrap();
                if active {
                    match frame["event"].as_str().unwrap() {
                        "poll" | "paused" => {
                            if listening.observe(
                                frame["previous"].as_f64().unwrap(),
                                time,
                                frame["playing"] == true,
                            ) && settings.should_scrobble(listening.seconds, 100.)
                            {
                                listening.submitted = true;
                            }
                        }
                        "seek" => listening.seek(time),
                        "reset" => listening.reset(time),
                        "force" => listening.submitted = true,
                        _ => unreachable!(),
                    }
                }
                assert!(
                    (listening.seconds * 1000. - frame["listened"].as_f64().unwrap()).abs() < 1e-7,
                    "{frame}"
                );
                assert_eq!(listening.submitted, frame["submitted"] == true, "{frame}");
            }
        }
        for case in reference["controls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["state"] != "private" && case["state"] != "disabled")
        {
            let listening = Listening {
                seconds: case["listened"].as_f64().unwrap(),
                submitted: case["state"] == "submitted",
                ..Default::default()
            };
            let settings = feishin_core::storage::Settings {
                scrobble_mode: serde_json::from_value(case["mode"].clone()).unwrap(),
                scrobble_seconds: 60.,
                scrobble_percentage: 50.,
                ..Default::default()
            };
            let actual = conditions(&listening, 100., &settings);
            assert_eq!(
                actual
                    .iter()
                    .map(|(label, _)| label.as_str())
                    .collect::<Vec<_>>(),
                case["texts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|label| label.as_str().unwrap())
                    .collect::<Vec<_>>()
            );
            for ((_, actual), expected) in actual.iter().zip(case["progress"].as_array().unwrap()) {
                assert!((actual - expected.as_f64().unwrap()).abs() < 1e-10);
            }
        }
    }
    #[test]
    fn report_window_preserves_original_shuffle_order_duplicates_and_limit() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../feishin-core/assets/scrobble-protocol-reference.json"
        ))
        .unwrap();
        for case in cases {
            let count = case["count"].as_u64().unwrap() as usize;
            let mut queue = feishin_core::queue::Queue::default();
            queue.add(
                (0..count)
                    .map(|index| Item {
                        id: format!("song{}", index % 3),
                        ..Default::default()
                    })
                    .collect(),
                feishin_core::queue::Enqueue::Now,
            );
            queue.order.reverse();
            queue.shuffle = true;
            queue.current = queue.order.first().copied();
            assert_eq!(
                queue_window(&queue),
                case["queue"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|entry| entry["Id"].as_str().unwrap().to_owned())
                    .collect::<Vec<_>>()
            );
            queue.current = None;
            assert!(queue_window(&queue).is_empty());
        }
    }
    #[gpui::test]
    fn transport_resume_preserves_listening_but_stop_and_duplicate_entry_changes_reset_it(
        cx: &mut TestAppContext,
    ) {
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc};
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-listening-resume-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = feishin_core::storage::State::default();
            state.settings.confirm_large_fetch = false;
            state.queue.add(
                vec![
                    Item {
                        id: "song".into(),
                        kind: ItemKind::Song,
                        duration: 100.,
                        ..Default::default()
                    };
                    2
                ],
                feishin_core::queue::Enqueue::Now,
            );
            state.queue.position = 20.;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.begin_track_reports(cx);
                view.listening.seconds = 12.;
                view.listening.submitted = true;
                view.stop_transport(cx);
                view.begin_track_reports(cx);
                assert_eq!(view.listening.seconds, 12.);
                assert!(view.listening.submitted);
                view.stop_playback(cx);
                view.begin_track_reports(cx);
                assert_eq!(view.listening.seconds, 0.);
                assert!(!view.listening.submitted);
                view.listening.seconds = 7.;
                view.listening.submitted = true;
                view.state.queue.current = Some(view.state.queue.entries[1].key);
                view.begin_track_reports(cx);
                assert_eq!(view.listening.seconds, 0.);
                assert!(!view.listening.submitted);
                view.state.settings.private_mode = true;
                view.poll_track_reports(cx);
                assert!(view.track_reports.pending.is_none());
            })
        });
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
    fn scrobble_hover_panel_matches_original_states_locales_and_manual_controls(
        cx: &mut TestAppContext,
    ) {
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc, time::Duration};
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-scrobble-ui-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let mut state = feishin_core::storage::State::default();
            state.settings.confirm_large_fetch = false;
            state.settings.scrobble_seconds = 60.;
            state.settings.scrobble_percentage = 50.;
            state.settings.playerbar_open_drawer = true;
            state.queue.add(
                vec![
                    Item {
                        id: "song".into(),
                        kind: ItemKind::Song,
                        duration: 100.,
                        ..Default::default()
                    };
                    2
                ],
                feishin_core::queue::Enqueue::Now,
            );
            state.queue.position = 42.;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let original = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        let target = cx.debug_bounds("scrobble-status").unwrap();
        cx.simulate_mouse_move(target.center(), None, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(499));
        cx.run_until_parked();
        assert!(cx.debug_bounds("scrobble-panel").is_none());
        cx.executor().advance_clock(Duration::from_millis(2));
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("scrobble-panel").unwrap().size.width,
            px(280.)
        );
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/scrobble-status-reference.json")).unwrap();
        for language in ["en", "fr", "ar"] {
            for case in reference["controls"].as_array().unwrap() {
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        view.state.settings.language = language.into();
                        view.state.settings.scrobble_mode =
                            serde_json::from_value(case["mode"].clone()).unwrap();
                        view.state.settings.scrobble = case["state"] != "disabled";
                        view.state.settings.private_mode = case["state"] == "private";
                        view.state.queue.entries[0].item.id =
                            if case["state"] == "empty" { "" } else { "song" }.into();
                        view.listening.seconds = case["listened"].as_f64().unwrap();
                        view.listening.submitted = case["state"] == "submitted";
                        view.apply_theme(window, cx);
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                cx.update(|window, _| {
                    let tree = window.accessibility_tree_for_test().unwrap();
                    for label in case["texts"].as_array().unwrap() {
                        let label = label.as_str().unwrap();
                        let label = feishin_core::i18n::translate(language, label)
                            .unwrap_or_else(|| label.into());
                        assert!(
                            tree.nodes
                                .iter()
                                .any(|(_, node)| node.value() == Some(label.as_str())
                                    || node.label() == Some(label.as_str())),
                            "Missing {label}: {case}"
                        );
                    }
                    for button in case["buttons"].as_array().unwrap() {
                        let label = feishin_core::i18n::translate(
                            language,
                            button["label"].as_str().unwrap(),
                        )
                        .unwrap();
                        let node = tree
                            .nodes
                            .iter()
                            .find(|(_, node)| {
                                node.role() == Role::Button && node.label() == Some(label.as_str())
                            })
                            .unwrap();
                        assert_eq!(node.1.is_disabled(), button["disabled"] == true, "{case}");
                    }
                });
            }
        }
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.state.queue.entries[0].item.id = "song".into();
                view.state.settings.scrobble = true;
                view.state.settings.private_mode = false;
                view.listening.seconds = 25.;
                view.listening.submitted = false;
                cx.notify();
            })
        });
        cx.run_until_parked();
        for (id, submitted, seconds) in [
            ("scrobble-submit", true, 25.),
            ("scrobble-reset", false, 0.),
        ] {
            let bounds = cx.debug_bounds(id).unwrap();
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
            app.read_with(cx, |view, _| {
                assert_eq!(view.listening.submitted, submitted);
                assert_eq!(view.listening.seconds, seconds);
                assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), original);
                assert!(!view.full_player);
            });
        }
        let _ = std::fs::remove_dir_all(dir);
    }
    #[gpui::test]
    fn successful_scrobble_updates_duplicates_and_resumes_only_the_same_playing_entry(
        cx: &mut TestAppContext,
    ) {
        use feishin_core::{
            api::Api,
            model::{Server, Session},
            queue::Enqueue,
            storage::State,
        };
        use gpui_component::Root;
        use std::{
            cell::RefCell,
            io::Write,
            net::TcpListener,
            rc::Rc,
            sync::mpsc,
            thread,
            time::{Duration, Instant},
        };
        cx.update(gpui_component::init);
        for (playing, switch_track) in [(true, false), (false, false), (true, true)] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let server = Server {
                id: uuid::Uuid::new_v4(),
                kind: ServerType::Jellyfin,
                url: format!("http://{}", listener.local_addr().unwrap()),
                name: "Fixture".into(),
                username: "fixture".into(),
                legacy_auth: false,
                options: Default::default(),
            };
            let (requests_tx, requests) = mpsc::channel();
            let (reply, reply_rx) = mpsc::channel();
            let worker = thread::spawn(move || {
                for index in 0..if playing && !switch_track { 2 } else { 1 } {
                    let (mut stream, request) = fixture_request(&listener);
                    requests_tx.send(request).unwrap();
                    if index == 0 {
                        reply_rx.recv_timeout(Duration::from_secs(8)).unwrap();
                    }
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                        )
                        .unwrap();
                }
            });
            let dir = std::env::temp_dir()
                .join(format!("feishin-scrobble-submit-{}", uuid::Uuid::new_v4()));
            let captured = Rc::new(RefCell::new(None));
            let (_, cx) = cx.add_window_view(|window, cx| {
                let mut state = State::default();
                state.settings.confirm_large_fetch = false;
                state.settings.scrobble = true;
                let song = Item {
                    id: "song".into(),
                    kind: ItemKind::Song,
                    server_id: server.id,
                    duration: 100.,
                    resume_position: 1.,
                    ..Default::default()
                };
                state.queue.add(
                    vec![
                        song.clone(),
                        song.clone(),
                        Item {
                            server_id: uuid::Uuid::new_v4(),
                            ..song
                        },
                    ],
                    Enqueue::Now,
                );
                state.queue.position = 42.;
                let app = cx.new(|cx| {
                    let mut view = Feishin::new(dir.clone(), Ok(state), window, cx);
                    view.sessions.insert(
                        server.id,
                        Api::new(
                            server.clone(),
                            Session {
                                token: "fixture".into(),
                                user_id: "user".into(),
                                ..Default::default()
                            },
                        )
                        .unwrap(),
                    );
                    view.playing = playing;
                    view.loaded = true;
                    view
                });
                captured.replace(Some(app.clone()));
                Root::new(app, window, cx)
            });
            let app = captured.borrow_mut().take().unwrap();
            cx.run_until_parked();
            cx.update(|_, cx| app.update(cx, |view, cx| view.submit_scrobble(cx)));
            let started = Instant::now();
            let first = loop {
                cx.run_until_parked();
                if let Ok(request) = requests.try_recv() {
                    break request;
                }
                assert!(started.elapsed() < Duration::from_secs(8));
                thread::sleep(Duration::from_millis(5));
            };
            assert!(first.starts_with("POST /Sessions/Playing/Stopped "));
            let body: serde_json::Value =
                serde_json::from_str(first.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["PositionTicks"], 1_000_000_000u64);
            app.read_with(cx, |view, _| {
                assert!(view.listening.submitted);
                assert!(
                    view.state
                        .queue
                        .entries
                        .iter()
                        .all(|entry| entry.item.play_count == 0)
                );
            });
            if switch_track {
                cx.update(|_, cx| {
                    app.update(cx, |view, cx| {
                        view.state.queue.current = Some(view.state.queue.entries[1].key);
                        cx.notify();
                    })
                });
            }
            reply.send(()).unwrap();
            let started = Instant::now();
            loop {
                cx.run_until_parked();
                if app.read_with(cx, |view, _| view.state.queue.entries[0].item.play_count) == 1 {
                    break;
                }
                assert!(started.elapsed() < Duration::from_secs(8));
                thread::sleep(Duration::from_millis(5));
            }
            app.read_with(cx, |view, _| {
                assert_eq!(
                    view.state
                        .queue
                        .entries
                        .iter()
                        .map(|entry| entry.item.play_count)
                        .collect::<Vec<_>>(),
                    vec![1, 1, 0]
                );
                assert_eq!(view.state.queue.position, 42.);
            });
            if playing && !switch_track {
                let started = Instant::now();
                let resumed = loop {
                    cx.run_until_parked();
                    if let Ok(request) = requests.try_recv() {
                        break request;
                    }
                    assert!(started.elapsed() < Duration::from_secs(8));
                    thread::sleep(Duration::from_millis(5));
                };
                assert!(
                    resumed.starts_with("POST /Sessions/Playing/Progress "),
                    "Unexpected report: {}",
                    resumed.lines().next().unwrap().split('?').next().unwrap()
                );
                let body: serde_json::Value =
                    serde_json::from_str(resumed.split_once("\r\n\r\n").unwrap().1).unwrap();
                assert_eq!(body["EventName"], "unpause");
                assert_eq!(body["PositionTicks"], 420_000_000u64);
                assert_eq!(body["IsPaused"], false);
            }
            worker.join().unwrap();
            let saved = feishin_core::storage::load(&dir.join("state.json")).unwrap();
            assert_eq!(saved.queue.entries[0].item.play_count, 1);
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    #[gpui::test]
    fn jellyfin_seek_throttle_and_book_progress_follow_original_playback_positions(
        cx: &mut TestAppContext,
    ) {
        use feishin_core::{
            api::Api,
            model::{Server, Session},
            playback::Event,
            queue::Enqueue,
            storage::State,
        };
        use gpui_component::Root;
        use std::{
            cell::RefCell,
            io::Write,
            net::TcpListener,
            rc::Rc,
            sync::mpsc,
            thread,
            time::{Duration, Instant},
        };
        cx.update(gpui_component::init);
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/scrobble-lifecycle-reference.json"))
                .unwrap();
        for case in reference
            .into_iter()
            .filter(|case| case["server"] == "jellyfin")
        {
            let frames = case["frames"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|frame| {
                    let at = frame["at"].as_u64().unwrap();
                    (2100..=4500).contains(&at)
                })
                .collect::<Vec<_>>();
            let expected = frames
                .iter()
                .flat_map(|frame| frame["calls"].as_array().unwrap())
                .cloned()
                .collect::<Vec<_>>();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let server = Server {
                id: uuid::Uuid::new_v4(),
                kind: ServerType::Jellyfin,
                url: format!("http://{}", listener.local_addr().unwrap()),
                name: "Fixture".into(),
                username: "fixture".into(),
                legacy_auth: false,
                options: Default::default(),
            };
            let (requests_tx, requests) = mpsc::channel();
            let count = expected.len();
            let (request_start, request_ready) = mpsc::channel();
            let worker = thread::spawn(move || {
                for _ in 0..count {
                    if request_ready.recv().is_err() {
                        return;
                    }
                    let (mut stream, request) = fixture_request(&listener);
                    requests_tx.send(request).unwrap();
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                        )
                        .unwrap();
                }
            });
            let dir = std::env::temp_dir()
                .join(format!("feishin-scrobble-clock-{}", uuid::Uuid::new_v4()));
            let captured = Rc::new(RefCell::new(None));
            let (sender, receiver) = mpsc::channel();
            let (_, cx) = cx.add_window_view(|window, cx| {
                let mut state = State::default();
                state.settings.scrobble = true;
                state.settings.confirm_large_fetch = false;
                state.settings.scrobble_seconds = 60.;
                state.settings.scrobble_percentage = 50.;
                state.queue.add(
                    vec![Item {
                        id: "song-0".into(),
                        kind: ItemKind::Song,
                        server_id: server.id,
                        duration: 100.,
                        resumable: case["resumable"] == true,
                        ..Default::default()
                    }],
                    Enqueue::Now,
                );
                let app = cx.new(|cx| {
                    let mut view = Feishin::new(dir.clone(), Ok(state), window, cx);
                    view.sessions.insert(
                        server.id,
                        Api::new(
                            server.clone(),
                            Session {
                                token: "fixture".into(),
                                user_id: "user".into(),
                                ..Default::default()
                            },
                        )
                        .unwrap(),
                    );
                    view.playing = true;
                    view.loaded = true;
                    view.player.events = receiver;
                    view
                });
                captured.replace(Some(app.clone()));
                Root::new(app, window, cx)
            });
            let app = captured.borrow_mut().take().unwrap();
            cx.run_until_parked();
            let mut clock = 0;
            let mut received = 0;
            for frame in frames {
                for _ in frame["calls"].as_array().unwrap() {
                    request_start.send(()).unwrap();
                }
                let at = frame["at"].as_u64().unwrap();
                cx.executor()
                    .advance_clock(Duration::from_millis(at - clock));
                clock = at;
                cx.run_until_parked();
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| match frame["event"].as_str().unwrap() {
                        "seek" => view.seek_position(frame["value"].as_f64().unwrap(), window, cx),
                        "poll" => {
                            sender
                                .send(Event::Position(
                                    view.player_generation,
                                    frame["value"].as_f64().unwrap(),
                                ))
                                .unwrap();
                            view.tick(window, cx);
                        }
                        "pause" => {
                            view.playing = false;
                            view.report("pause", cx);
                        }
                        "play" => {
                            view.playing = true;
                            view.report("unpause", cx);
                        }
                        _ => unreachable!(),
                    })
                });
                for expected in frame["calls"].as_array().unwrap() {
                    let started = Instant::now();
                    let request = loop {
                        cx.run_until_parked();
                        if let Ok(request) = requests.try_recv() {
                            break request;
                        }
                        assert!(
                            started.elapsed() < Duration::from_secs(8),
                            "Missing {expected}"
                        );
                        thread::sleep(Duration::from_millis(5));
                    };
                    let body: serde_json::Value =
                        serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                    assert_eq!(body["EventName"], expected["event"]);
                    assert_eq!(body["PositionTicks"], expected["position"]);
                    if expected["event"] != "timeupdate" {
                        assert_eq!(body["IsPaused"], expected["event"] == "pause");
                    }
                    received += 1;
                }
            }
            assert_eq!(received, expected.len());
            worker.join().unwrap();
            assert!(requests.try_recv().is_err());
            let _ = std::fs::remove_dir_all(dir);
        }
    }
    #[gpui::test]
    fn submitted_scrobbles_refresh_metadata_without_resetting_album_scroll_or_artist_selection(
        cx: &mut TestAppContext,
    ) {
        use feishin_core::{
            api::Api,
            model::{Server, Session},
            queue::Enqueue,
            storage::State,
        };
        use gpui_component::Root;
        use std::{
            cell::RefCell,
            io::Write,
            net::TcpListener,
            rc::Rc,
            thread,
            time::{Duration, Instant},
        };
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-scrobble-refresh-{}", uuid::Uuid::new_v4()));
        let server_id = uuid::Uuid::new_v4();
        let song = Item {
            id: "song".into(),
            album_id: "album".into(),
            kind: ItemKind::Song,
            server_id,
            duration: 100.,
            ..Default::default()
        };
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.confirm_large_fetch = false;
            state.queue.add(vec![song.clone()], Enqueue::Now);
            state.queue.position = 20.;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.state.active_server = Some(server_id);
                view.route = Route::Detail(Box::new(Item {
                    id: "album".into(),
                    kind: ItemKind::Album,
                    server_id,
                    ..Default::default()
                }));
                view.album_metadata_extra = px(80.);
                view.page_scroll.set_offset(point(px(0.), px(-400.)));
                view.items = vec![song.clone()];
                view.selected.insert(0);
                view.scrobble_succeeded(&song, cx);
                assert_eq!(view.album_metadata_extra, px(80.));
                assert_eq!(view.page_scroll.offset(), point(px(0.), px(-400.)));
                assert_eq!(view.items.len(), 1);
                assert!(view.selected.contains(&0));
                assert_eq!(view.state.queue.position, 20.);
            })
        });
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = Server {
            id: server_id,
            kind: ServerType::Jellyfin,
            url: format!("http://{}", listener.local_addr().unwrap()),
            name: "Fixture".into(),
            username: "fixture".into(),
            legacy_auth: false,
            options: Default::default(),
        };
        let worker = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = fixture_request(&listener);
                let body = r#"{"Items":[{"Id":"song","Name":"Song","Type":"Audio","RunTimeTicks":1000000000,"UserData":{"IsFavorite":true,"PlayCount":1}}],"TotalRecordCount":1}"#;
                write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            }
        });
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Detail(Box::new(Item {
                    id: "artist".into(),
                    kind: ItemKind::AlbumArtist,
                    server_id,
                    ..Default::default()
                }));
                view.artist_overview = true;
                view.sessions.insert(
                    server_id,
                    Api::new(
                        server,
                        Session {
                            token: "fixture".into(),
                            user_id: "user".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                );
                for name in ["topSongs", "favoriteSongs"] {
                    let section = crate::artist_overview::Section {
                        items: vec![song.clone()],
                        selected: ["song".into()].into(),
                        selection_order: vec!["song".into()],
                        ..Default::default()
                    };
                    section
                        .horizontal_scroll
                        .set_offset(point(px(-80.), px(0.)));
                    view.artist_sections.insert(name.into(), section);
                }
                view.scrobble_succeeded(&song, cx);
                for name in ["topSongs", "favoriteSongs"] {
                    let section = &view.artist_sections[name];
                    assert!(section.selected.contains("song"));
                    assert_eq!(section.selection_order, ["song"]);
                    assert_eq!(section.horizontal_scroll.offset(), point(px(-80.), px(0.)));
                }
            })
        });
        let started = Instant::now();
        loop {
            cx.run_until_parked();
            if app.read_with(cx, |view, _| {
                ["topSongs", "favoriteSongs"]
                    .iter()
                    .all(|name| !view.artist_sections[*name].loading)
            }) {
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(8));
            thread::sleep(Duration::from_millis(5));
        }
        worker.join().unwrap();
        app.read_with(cx, |view, _| {
            for name in ["topSongs", "favoriteSongs"] {
                let section = &view.artist_sections[name];
                assert!(section.error.is_empty(), "{}", section.error);
                assert!(section.selected.contains("song"));
                assert_eq!(section.selection_order, ["song"]);
            }
            assert_eq!(view.state.queue.position, 20.);
        });
        let _ = std::fs::remove_dir_all(dir);
    }
    #[gpui::test]
    fn scrobble_condition_bars_match_original_rtl_layout_and_width_transition(
        cx: &mut TestAppContext,
    ) {
        struct Probe {
            progress: f32,
            rtl: bool,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().w(rems(12.5)).child(ConditionBar {
                    index: 0,
                    progress: self.progress,
                    rtl: self.rtl,
                })
            }
        }
        cx.update(gpui_component::init);
        let (probe, cx) = cx.add_window_view(|_, _| Probe {
            progress: 30.,
            rtl: false,
        });
        let reference = crate::progress::layout_reference();
        for rtl in [false, true] {
            cx.update(|window, cx| {
                window.set_rem_size(px(16.));
                probe.update(cx, |probe, cx| {
                    probe.progress = 30.;
                    probe.rtl = rtl;
                    cx.notify();
                });
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(100));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            probe.update(cx, |probe, cx| {
                probe.progress = 70.;
                cx.notify();
            });
            cx.run_until_parked();
            let mut previous = 0;
            for sample in reference["motion"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["name"] == "progress")
            {
                let ms = sample["elapsed"].as_u64().unwrap();
                cx.executor()
                    .advance_clock(std::time::Duration::from_millis(ms - previous));
                previous = ms;
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
                let expected: f32 = sample["value"]
                    .as_str()
                    .unwrap()
                    .trim_end_matches("px")
                    .parse()
                    .unwrap();
                let bounds = cx.debug_bounds("scrobble-condition-fill-0").unwrap();
                assert!(
                    (f32::from(bounds.size.width) - expected).abs() < 1.1,
                    "Width at {ms}ms: {bounds:?} != {expected}"
                );
                assert!(
                    (f32::from(if rtl { bounds.right() } else { bounds.left() })
                        - if rtl { 200. } else { 0. })
                    .abs()
                        < 1.1
                );
                assert_eq!(bounds.size.height, px(3.));
            }
        }
    }
}
