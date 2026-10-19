#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("This fixture requires an isolated Wayland compositor");
}

#[cfg(target_os = "linux")]
fn main() {
    use gpui::{prelude::*, *};
    use raw_window_handle::HasWindowHandle;
    use std::time::Duration;

    struct Probe(usize);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().bg(rgb(if self.0.is_multiple_of(2) {
                0x2266aa
            } else {
                0xaa6622
            }))
        }
    }
    Application::new().run(|cx| {
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(320.), px(240.)),
                    ))),
                    window_min_size: Some(size(px(160.), px(120.))),
                    ..Default::default()
                },
                |window, cx| {
                    assert!(matches!(
                        HasWindowHandle::window_handle(window).unwrap().as_raw(),
                        raw_window_handle::RawWindowHandle::Wayland(_)
                    ));
                    window.set_window_title("Feishin Wayland fixture");
                    window.set_app_id("org.feishin.wayland-fixture");
                    cx.new(|_| Probe(0))
                },
            )
            .unwrap();
        cx.spawn(async move |cx| {
            smol::Timer::after(Duration::from_millis(500)).await;
            for cycle in 0..4 {
                eprintln!("FEISHIN_WAYLAND_HIDING {cycle}");
                handle
                    .update(cx, |_, window, _| {
                        assert!(window.set_visible(false));
                        assert!(window.set_visible(false));
                    })
                    .unwrap();
                eprintln!("FEISHIN_WAYLAND_HIDDEN {cycle}");
                // Keep invalidating the retained view while hidden. Drawing must
                // not attach a buffer and accidentally remap the surface.
                for _ in 0..8 {
                    smol::Timer::after(Duration::from_millis(20)).await;
                    handle
                        .update(cx, |view, _, cx| {
                            view.0 += 1;
                            cx.notify();
                        })
                        .unwrap();
                }
                eprintln!("FEISHIN_WAYLAND_SHOWING {cycle}");
                handle
                    .update(cx, |_, window, _| {
                        assert!(window.set_visible(true));
                        assert!(window.set_visible(true));
                    })
                    .unwrap();
                smol::Timer::after(Duration::from_millis(300)).await;
                handle
                    .update(cx, |view, _, _| assert_eq!(view.0, (cycle + 1) * 8))
                    .unwrap();
            }
            eprintln!("FEISHIN_WAYLAND_RAPID_START");
            handle
                .update(cx, |view, window, cx| {
                    // Restore requests may arrive while earlier configure events
                    // are still in flight. End visible, retaining the same view.
                    for _ in 0..8 {
                        assert!(window.set_visible(false));
                        assert!(window.set_visible(true));
                    }
                    view.0 += 1;
                    cx.notify();
                })
                .unwrap();
            eprintln!("FEISHIN_WAYLAND_RAPID_END");
            smol::Timer::after(Duration::from_millis(300)).await;
            handle
                .update(cx, |view, _, _| assert_eq!(view.0, 33))
                .unwrap();
            eprintln!("FEISHIN_WAYLAND_PASSED");
            cx.update(|cx| cx.quit()).unwrap();
        })
        .detach();
    });
}
