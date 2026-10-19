use crate::app::Feishin;
use feishin_core::api::{Api, ScanStatus};
use gpui::Context;
use std::time::Duration;
use uuid::Uuid;

#[derive(Default)]
pub struct State {
    pub generation: u64,
    pub server: Option<Uuid>,
    pub starting: bool,
    pub watching: bool,
    seen_start: bool,
    polls: u32,
    previous_end: Option<String>,
    pub status: ScanStatus,
}

impl State {
    fn observe(&mut self, status: ScanStatus) -> bool {
        self.polls += 1;
        self.seen_start |= status.scanning;
        let finished = !status.scanning
            && (self.seen_start
                || self.polls >= 15
                || (self.previous_end.is_some()
                    && status.last_scan.is_some()
                    && status.last_scan != self.previous_end));
        self.status = status;
        finished
    }
}

impl Feishin {
    pub fn reset_library_scan(&mut self) {
        self.library_scan = State {
            generation: self.library_scan.generation.wrapping_add(1),
            ..Default::default()
        };
    }
    pub fn library_scan_busy(&self) -> bool {
        self.library_scan.server == self.state.active_server
            && (self.library_scan.starting || self.library_scan.watching)
    }

    pub fn can_scan_library(&self) -> bool {
        self.api().is_some_and(|api| api.session().is_admin)
    }

    pub fn library_scan_label(&self) -> String {
        if self.library_scan_busy() {
            let count = self.library_scan.status.count;
            if count > 0 {
                format!("{}: {count}", self.tr("common.scanningLibrary"))
            } else {
                self.tr("common.scanningLibrary")
            }
        } else {
            self.tr("page.appMenu.rescanLibrary")
        }
    }

    pub fn start_library_scan(&mut self, cx: &mut Context<Self>) {
        if self.library_scan_busy() || !self.can_scan_library() {
            return;
        }
        let Some(api) = self.api() else {
            return;
        };
        let server = api.server.id;
        let generation = self.library_scan.generation.wrapping_add(1);
        self.library_scan = State {
            generation,
            server: Some(server),
            starting: true,
            ..Default::default()
        };
        let request_api = api.clone();
        let task = cx.background_executor().spawn(async move {
            let previous = request_api.scan_status().ok();
            request_api.start_scan().map(|()| previous)
        });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                if view.library_scan.generation != generation
                    || view.state.active_server != Some(server)
                {
                    return;
                }
                view.library_scan.starting = false;
                match result {
                    Ok(previous) => view.watch_library_scan(api, previous, cx),
                    Err(error) => {
                        log::warn!("Library scan request failed: {error}");
                        view.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn watch_library_scan(
        &mut self,
        api: Api,
        previous: Option<ScanStatus>,
        cx: &mut Context<Self>,
    ) {
        if self.state.active_server != Some(api.server.id) {
            return;
        }
        let generation = self.library_scan.generation.wrapping_add(1);
        let server = api.server.id;
        self.library_scan = State {
            generation,
            server: Some(server),
            watching: true,
            previous_end: previous.and_then(|status| status.last_scan),
            ..Default::default()
        };
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                if !view
                    .read_with(cx, |view, _| {
                        view.library_scan.generation == generation
                            && view.library_scan.watching
                            && view.state.active_server == Some(server)
                            && view.api().is_some()
                    })
                    .unwrap_or(false)
                {
                    break;
                }
                let api = api.clone();
                let task = cx
                    .background_executor()
                    .spawn(async move { api.scan_status() });
                let result = task.await;
                let keep_watching = view
                    .update(cx, |view, cx| {
                        view.receive_scan_status(server, generation, result, cx)
                    })
                    .unwrap_or(false);
                if !keep_watching {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn receive_scan_status(
        &mut self,
        server: Uuid,
        generation: u64,
        result: anyhow::Result<ScanStatus>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.state.active_server != Some(server) || self.library_scan.generation != generation {
            return false;
        }
        match result {
            Ok(status) => {
                if self.library_scan.observe(status) {
                    self.library_scan.watching = false;
                    self.invalidate_library_data();
                    self.refresh_playlists(cx);
                    self.reload(false, cx);
                    self.message = self.tr("Library scan finished");
                }
            }
            Err(error) => {
                self.library_scan.watching = false;
                log::warn!("Library scan status failed: {error}");
                self.message =
                    format!("{}: {error}", self.tr("Unable to read library scan status"));
            }
        }
        cx.notify();
        self.library_scan.watching
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_watch_waits_for_delayed_start_detects_completion_and_bounds_unreported_scans() {
        let mut scan = State::default();
        assert!(!scan.observe(ScanStatus::default()));
        assert!(!scan.observe(ScanStatus {
            scanning: true,
            count: 3,
            ..Default::default()
        }));
        assert!(scan.observe(ScanStatus {
            count: 4,
            ..Default::default()
        }));
        assert_eq!(scan.status.count, 4);
        let mut scan = State::default();
        for _ in 0..14 {
            assert!(!scan.observe(ScanStatus::default()));
        }
        assert!(scan.observe(ScanStatus::default()));
        let mut scan = State {
            previous_end: Some("before".into()),
            ..Default::default()
        };
        assert!(!scan.observe(ScanStatus {
            last_scan: Some("before".into()),
            ..Default::default()
        }));
        assert!(scan.observe(ScanStatus {
            last_scan: Some("after".into()),
            ..Default::default()
        }));
    }
}
