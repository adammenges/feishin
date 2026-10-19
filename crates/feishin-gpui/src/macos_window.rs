use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSColor, NSWindow, NSWindowStyleMask,
};

pub fn apply_theme(window: &NSWindow, [r, g, b, a]: [f32; 4], dark: bool) {
    let background =
        NSColor::colorWithSRGBRed_green_blue_alpha(r.into(), g.into(), b.into(), a.into());
    window.setBackgroundColor(Some(&background));
    // Keep the system title and traffic lights, but use the app's background.
    if !window.styleMask().contains(NSWindowStyleMask::FullScreen) {
        window.setTitlebarAppearsTransparent(true);
    }
    // SAFETY: These are AppKit's constant appearance names.
    let name = unsafe {
        if dark {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        }
    };
    window.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
}
