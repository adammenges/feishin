//! The assertion belongs to one worker and is released on pause, reconfiguration, or exit.
use std::{sync::mpsc, thread};

pub struct Inhibitor {
    tx: mpsc::Sender<Option<bool>>,
    mode: Option<bool>,
}
impl Default for Inhibitor {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            worker(rx, |display| {
                keepawake::Builder::default()
                    .display(display)
                    .idle(true)
                    .reason("Music playback")
                    .app_name("Feishin")
                    .app_reverse_domain("org.jeffvli.feishin.rust")
                    .create()
            })
        });
        Self { tx, mode: None }
    }
}
impl Inhibitor {
    pub fn set(&mut self, playing: bool, prevent_sleep: bool, prevent_suspend: bool) {
        let mode = (playing && (prevent_sleep || prevent_suspend)).then_some(prevent_sleep);
        if mode != self.mode {
            self.mode = mode;
            let _ = self.tx.send(mode);
        }
    }
}
fn worker<T, E: std::fmt::Display>(
    rx: mpsc::Receiver<Option<bool>>,
    mut create: impl FnMut(bool) -> Result<T, E>,
) {
    let mut assertion = None;
    while let Ok(mode) = rx.recv() {
        // Keep only the latest playback state when several commands arrive together.
        let mode = rx.try_iter().last().unwrap_or(mode);
        assertion.take();
        if let Some(display) = mode {
            match create(display) {
                Ok(value) => assertion = Some(value),
                Err(error) => log::warn!("Could not inhibit sleep during playback: {error}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };
    #[test]
    fn playback_assertions_are_released_on_pause_reconfiguration_and_shutdown() {
        struct Assertion(Arc<AtomicUsize>);
        impl Drop for Assertion {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let count = Arc::new(AtomicUsize::new(0));
        let worker_count = count.clone();
        let (tx, rx) = mpsc::channel();
        let (created, events) = mpsc::channel();
        let thread = thread::spawn(move || {
            worker(rx, |display| {
                worker_count.fetch_add(1, Ordering::SeqCst);
                created.send(display).unwrap();
                Ok::<_, std::convert::Infallible>(Assertion(worker_count.clone()))
            })
        });
        let mut inhibitor = Inhibitor { tx, mode: None };
        inhibitor.set(false, true, true);
        inhibitor.set(true, false, true);
        assert!(!events.recv_timeout(Duration::from_secs(2)).unwrap());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        inhibitor.set(true, true, false);
        assert!(events.recv_timeout(Duration::from_secs(2)).unwrap());
        assert_eq!(count.load(Ordering::SeqCst), 1);
        inhibitor.set(false, true, true);
        drop(inhibitor);
        thread.join().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
