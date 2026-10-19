// AppKit windows require the real main thread, so this test has no libtest harness.
#[cfg(target_os = "macos")]
#[path = "../src/macos_window.rs"]
mod macos_window;

#[cfg(target_os = "macos")]
fn main() {
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSAppearanceCustomization, NSApplication, NSBackingStoreType, NSWindow, NSWindowStyleMask,
        NSWindowTitleVisibility,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let marker = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(marker);
    let system_appearance = app.effectiveAppearance().name();
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;
    // Never order the window onto the user's desktop or start the app event loop.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(marker),
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(640., 480.)),
            style,
            NSBackingStoreType::Buffered,
            true,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    let layout = window.contentLayoutRect();
    for (color, dark) in [(0x002b36, true), (0xfdf6e3, false), (0x002b36, true)] {
        let rgba = [
            ((color >> 16) & 255) as f32 / 255.,
            ((color >> 8) & 255) as f32 / 255.,
            (color & 255) as f32 / 255.,
            1.,
        ];
        macos_window::apply_theme(&window, rgba, dark);
        let background = window.backgroundColor();
        for (actual, expected) in [
            background.redComponent(),
            background.greenComponent(),
            background.blueComponent(),
            background.alphaComponent(),
        ]
        .into_iter()
        .zip(rgba)
        {
            assert!((actual - f64::from(expected)).abs() < 0.00001);
        }
        assert!(window.titlebarAppearsTransparent());
        assert_eq!(window.styleMask(), style);
        assert_eq!(window.titleVisibility(), NSWindowTitleVisibility::Visible);
        assert_eq!(window.contentLayoutRect(), layout);
        assert_eq!(
            window.effectiveAppearance().name().to_string(),
            if dark {
                "NSAppearanceNameDarkAqua"
            } else {
                "NSAppearanceNameAqua"
            }
        );
        assert_eq!(app.effectiveAppearance().name(), system_appearance);
        assert!(!window.isVisible());
        // SAFETY: The window and its frame view remain alive on the main thread.
        let frame = unsafe { window.contentView().unwrap().superview().unwrap() };
        frame.layoutSubtreeIfNeeded();
        let bitmap = frame
            .bitmapImageRepForCachingDisplayInRect(frame.bounds())
            .unwrap();
        frame.cacheDisplayInRect_toBitmapImageRep(frame.bounds(), &bitmap);
        // Sample clear areas above and below the titlebar/content boundary.
        let header = bitmap.colorAtX_y(bitmap.pixelsWide() * 3 / 4, 10).unwrap();
        let content = bitmap
            .colorAtX_y(bitmap.pixelsWide() * 3 / 4, bitmap.pixelsHigh() / 2)
            .unwrap();
        for (header, content) in [
            (header.redComponent(), content.redComponent()),
            (header.greenComponent(), content.greenComponent()),
            (header.blueComponent(), content.blueComponent()),
        ] {
            assert!(
                (header - content).abs() < 0.01,
                "Title bar must render the app background"
            );
        }
    }
    window.close();
    println!(
        "macOS title bar follows theme colors without changing system appearance or content layout"
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {}
