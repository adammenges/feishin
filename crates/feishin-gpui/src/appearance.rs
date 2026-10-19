use crate::app::Feishin;
use crate::scroll::VerticalScroll;
use feishin_core::{appearance, i18n};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Selectable, StyledExt, Theme, ThemeMode};

type OverlayEntry = (String, u32, u32, std::sync::Arc<RenderImage>);
#[derive(Default)]
pub struct OverlayCache(std::collections::HashMap<String, OverlayEntry>);

impl OverlayCache {
    pub(crate) fn get(
        &mut self,
        key: &str,
        signature: &str,
        gradient: &appearance::Gradient,
        width: u32,
        height: u32,
    ) -> std::sync::Arc<RenderImage> {
        if let Some((old, w, h, image)) = self.0.get(key)
            && old == signature
            && *w == width
            && *h == height
        {
            return image.clone();
        }
        let image = gradient_image(gradient, width, height, width as f32, height as f32);
        self.0
            .insert(key.into(), (signature.into(), width, height, image.clone()));
        image
    }
}

fn gradient_image(
    gradient: &appearance::Gradient,
    width: u32,
    height: u32,
    css_width: f32,
    css_height: f32,
) -> std::sync::Arc<RenderImage> {
    let width = width.clamp(1, 2048);
    let height = height.clamp(1, 2048);
    let mut pixels = gradient.raster_in_bounds(width, height, css_width, css_height);
    if gradient.noise {
        let noise = noise_image(width as f32 / css_width);
        for (index, rgba) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = index as u32 % width;
            let y = index as u32 / width;
            let under = noise.get_pixel(x % noise.width(), y % noise.height()).0;
            let a = rgba[3] as f32 / 255.;
            let b = under[3] as f32 / 255. * (1. - a);
            let alpha = a + b;
            if alpha > 0. {
                for i in 0..3 {
                    rgba[i] = ((rgba[i] as f32 * a + under[i] as f32 * b) / alpha).round() as u8;
                }
            }
            rgba[3] = (alpha * 255.).round() as u8;
        }
    }
    for rgba in pixels.as_chunks_mut::<4>().0 {
        rgba.swap(0, 2);
    }
    let image =
        image::RgbaImage::from_raw(width, height, pixels).expect("Gradient pixels are invalid");
    std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(image)]))
}

fn noise_image(scale: f32) -> std::sync::Arc<image::RgbaImage> {
    static NOISE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<u32, std::sync::Arc<image::RgbaImage>>>,
    > = std::sync::OnceLock::new();
    let width = (300. * scale).round().clamp(1., 2048.) as u32;
    let mut images = NOISE
        .get_or_init(Default::default)
        .lock()
        .expect("Noise image cache is poisoned");
    if let Some(image) = images.get(&width) {
        return image.clone();
    }
    let bytes = if width <= 300 {
        include_bytes!("../assets/theme-noise.png").as_slice()
    } else {
        include_bytes!("../assets/theme-noise-2x.png").as_slice()
    };
    let original = image::load_from_memory(bytes)
        .expect("Original theme noise is invalid")
        .to_rgba8();
    let image = std::sync::Arc::new(if original.width() == width {
        original
    } else {
        image::imageops::resize(
            &original,
            width,
            width,
            image::imageops::FilterType::Triangle,
        )
    });
    if images.len() >= 8 {
        images.clear();
    }
    images.insert(width, image.clone());
    image
}

pub(crate) fn gradient_fill(gradient: appearance::Gradient) -> ImageFill {
    let images = std::sync::Mutex::new(std::collections::VecDeque::<(
        Size<Pixels>,
        f32,
        std::sync::Arc<RenderImage>,
    )>::new());
    ImageFill::new(move |bounds, window| {
        let scale = window.scale_factor();
        let mut images = images.lock().expect("Gradient image cache is poisoned");
        if let Some(index) = images
            .iter()
            .position(|(size, old_scale, _)| *size == bounds && *old_scale == scale)
        {
            let entry = images.remove(index).unwrap();
            let image = std::sync::Arc::clone(&entry.2);
            images.push_back(entry);
            return image;
        }
        let css_width = f32::from(bounds.width).max(f32::EPSILON);
        let css_height = f32::from(bounds.height).max(f32::EPSILON);
        let image = gradient_image(
            &gradient,
            (css_width * scale).ceil().clamp(1., 2048.) as u32,
            (css_height * scale).ceil().clamp(1., 2048.) as u32,
            css_width,
            css_height,
        );
        if images.len() >= 8 {
            let (_, _, old) = images.pop_front().unwrap();
            if let Err(error) = window.drop_image(old) {
                log::warn!("Unable to release cached CSS gradient: {error}");
            }
        }
        images.push_back((bounds, scale, image.clone()));
        image
    })
}

pub struct Language(pub String);
impl Global for Language {}

pub fn text(english: &str, cx: &App) -> String {
    i18n::text(
        cx.try_global::<Language>()
            .map_or("en", |locale| locale.0.as_str()),
        english,
    )
}

pub fn input_state(
    window: &mut Window,
    cx: &mut Context<gpui_component::input::InputState>,
) -> gpui_component::input::InputState {
    gpui_component::input::InputState::new(window, cx)
        .label_transform(|label, cx| text(label, cx).into())
}

pub fn rtl(cx: &App) -> bool {
    cx.try_global::<Language>()
        .is_some_and(|locale| i18n::rtl(&locale.0))
}

impl Feishin {
    pub fn tr(&self, english: &str) -> String {
        i18n::text(&self.state.settings.language, english)
    }

    pub fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_audio_preset_options(window, cx);
        self.sync_css_file();
        let selected = self.selected_theme(matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ));
        let result = appearance::resolve(selected, &self.data_dir.join("Themes"));
        let palette_signature = match &result {
            Ok(palette) => format!(
                "{}-{:?}",
                serde_json::to_string(palette).unwrap_or_default(),
                palette.invalid_color_keys
            ),
            Err(error) => error.to_string(),
        };
        let signature = format!(
            "{palette_signature}-{:?}-{}-{}-{}-{}-{}-{}-{:?}",
            self.state.settings.font,
            self.state.settings.zoom,
            crate::fonts::file_signature(self.state.settings.font.custom.as_deref()),
            self.state.settings.accent,
            self.state.settings.use_theme_accent_color,
            self.state.settings.use_theme_primary_shade,
            self.state.settings.primary_shade,
            self.state.settings.css
        );
        if signature == self.theme_signature {
            return;
        }
        if let Ok(palette) = &result
            && let Some(warning) = palette.color_warning()
        {
            log::warn!("Custom theme {selected:?}: {warning}");
        }
        self.theme_signature = signature;
        let mut palette = match result {
            Ok(palette) => palette,
            Err(error) => {
                self.message = error.to_string();
                cx.notify();
                return;
            }
        };
        let mut css_variables =
            appearance::css_variables_with_settings(&palette, &self.state.settings);
        let [r, g, b, a] = appearance::primary_contrast(&palette, &self.state.settings)
            .unwrap_or([1., 1., 1., 1.]);
        let primary_foreground: Hsla = Rgba { r, g, b, a }.into();
        match appearance::primary(&palette, &self.state.settings) {
            Ok([r, g, b, _]) => {
                palette.colors.insert(
                    "primary".into(),
                    format!(
                        "#{:02x}{:02x}{:02x}",
                        (r * 255.).round() as u8,
                        (g * 255.).round() as u8,
                        (b * 255.).round() as u8
                    ),
                );
            }
            Err(error) => {
                self.message = format!("Invalid accent color: {error}");
                return;
            }
        }
        Theme::change(
            if palette.mode == "light" {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            },
            Some(window),
            cx,
        );
        let theme = Theme::global_mut(cx);
        let mut warnings = vec![];
        for (key, value) in &palette.colors {
            let [r, g, b, a] = match appearance::color(value) {
                Ok(color) => color,
                Err(_) => {
                    warnings.push(format!("Invalid theme color: {key}"));
                    continue;
                }
            };
            let color: Hsla = Rgba { r, g, b, a }.into();
            match key.as_str() {
                "background" => {
                    theme.background = color;
                    theme.sidebar = color;
                    theme.list = color;
                    theme.table = color;
                    theme.tiles = color;
                    theme.title_bar = color;
                }
                "background-alternate" => {
                    theme.muted = color;
                    theme.sidebar_accent = color;
                    theme.accordion = color;
                    theme.accordion_hover = color;
                    theme.list_even = color;
                    theme.list_head = color;
                    theme.table_even = color;
                    theme.table_head = color;
                    theme.tab_bar = color;
                    theme.tab_bar_segmented = color;
                    theme.skeleton = color;
                    theme.switch = color;
                }
                "surface" => {
                    theme.secondary = color;
                    theme.secondary_hover = color;
                    theme.secondary_active = color;
                    theme.accent = color;
                    theme.popover = color;
                    theme.group_box = color;
                    theme.description_list_label = color;
                    theme.tab = color;
                    theme.tab_active = color;
                    theme.list_hover = color;
                    theme.list_active = color;
                    theme.table_hover = color;
                    theme.table_active = color;
                }
                "surface-foreground" => {
                    theme.secondary_foreground = color;
                    theme.accent_foreground = color;
                    theme.popover_foreground = color;
                    theme.group_box_foreground = color;
                    theme.description_list_label_foreground = color;
                    theme.tab_active_foreground = color;
                    theme.table_head_foreground = color;
                    theme.sidebar_accent_foreground = color;
                }
                "foreground" => {
                    theme.foreground = color;
                    theme.sidebar_foreground = color;
                    theme.tab_foreground = color;
                }
                "foreground-muted" => theme.muted_foreground = color,
                "border" => {
                    theme.border = color;
                    theme.input = color;
                    theme.sidebar_border = color;
                    theme.table_row_border = color;
                    theme.title_bar_border = color;
                    theme.window_border = color;
                }
                "primary" => {
                    theme.primary = color;
                    theme.primary_hover = color;
                    theme.primary_active = color;
                    theme.sidebar_primary = color;
                    theme.primary_foreground = primary_foreground;
                    theme.sidebar_primary_foreground = primary_foreground;
                    theme.ring = color;
                    theme.caret = color;
                    theme.drag_border = color;
                    theme.progress_bar = color;
                    theme.slider_bar = color;
                    theme.link = color;
                    theme.link_hover = color;
                    theme.link_active = color;
                    theme.list_active_border = color;
                    theme.table_active_border = color;
                    theme.selection = color.opacity(0.2);
                    theme.drop_target = color.opacity(0.2);
                }
                "state-error" => {
                    theme.danger = color;
                    theme.danger_hover = color;
                    theme.danger_active = color;
                    theme.danger_foreground = primary_foreground;
                    theme.bearish = color;
                }
                "state-info" => {
                    theme.info = color;
                    theme.info_hover = color;
                    theme.info_active = color;
                    theme.info_foreground = primary_foreground;
                }
                "state-success" => {
                    theme.success = color;
                    theme.success_hover = color;
                    theme.success_active = color;
                    theme.success_foreground = primary_foreground;
                    theme.bullish = color;
                }
                "state-warning" => {
                    theme.warning = color;
                    theme.warning_hover = color;
                    theme.warning_active = color;
                    theme.warning_foreground = primary_foreground;
                }
                "black" => theme.overlay = color.opacity(0.5),
                "white" => {
                    theme.switch_thumb = color;
                    theme.slider_thumb = color;
                }
                "red" => {
                    theme.red = color;
                    theme.red_light = color;
                }
                "green" => {
                    theme.green = color;
                    theme.green_light = color;
                    theme.chart_2 = color;
                }
                "blue" => {
                    theme.blue = color;
                    theme.blue_light = color;
                    theme.chart_1 = color;
                }
                "yellow" => {
                    theme.yellow = color;
                    theme.yellow_light = color;
                }
                "magenta" => {
                    theme.magenta = color;
                    theme.magenta_light = color;
                }
                "cyan" => {
                    theme.cyan = color;
                    theme.cyan_light = color;
                    theme.chart_5 = color;
                }
                "orange" => theme.chart_3 = color,
                "violet" => theme.chart_4 = color,
                _ => {}
            }
        }
        if let Some(font_size) = palette
            .app
            .get("root-font-size")
            .and_then(|value| value.trim_end_matches("px").parse::<f32>().ok())
            .filter(|n| n.is_finite() && (10.0..=32.0).contains(n))
        {
            theme.font_size = px(font_size);
        }
        theme.font_size *= self.state.settings.zoom;
        window
            .text_system()
            .set_optical_size_scale(self.state.settings.zoom);
        window.set_rem_size(theme.font_size);
        let colors = &mut theme.colors;
        for (key, target) in [
            ("scrollbar-handle-background", &mut colors.scrollbar_thumb),
            (
                "scrollbar-handle-hover-background",
                &mut colors.scrollbar_thumb_hover,
            ),
            ("scrollbar-track-background", &mut colors.scrollbar),
        ] {
            if let Some(value) = palette.app.get(key)
                && let Ok([r, g, b, a]) = appearance::color(value)
            {
                *target = Rgba { r, g, b, a }.into();
            }
        }
        self.apply_font(cx);
        let font_stack = format!(
            "\"{}\", \"Noto Sans JP\", \"Noto Sans Hebrew\", sans-serif",
            cx.theme()
                .font_family
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
        );
        palette
            .app
            .insert("content-font-family".into(), font_stack.clone());
        css_variables.insert("--theme-content-font-family".into(), font_stack);
        let preview_stylesheet = crate::context_preview::stylesheet();
        let content_mark_stylesheet = crate::ui::content_mark_stylesheet();
        let modal_stylesheet = crate::styles::modal_stylesheet();
        let button_stylesheet = crate::styles::button_stylesheet();
        let select_stylesheet = crate::styles::select_stylesheet();
        match feishin_core::styles::Stylesheet::parse(
            std::iter::once(preview_stylesheet.as_str())
                .chain(std::iter::once(content_mark_stylesheet.as_str()))
                .chain(std::iter::once(modal_stylesheet.as_str()))
                .chain(std::iter::once(button_stylesheet.as_str()))
                .chain(std::iter::once(select_stylesheet.as_str()))
                .chain(std::iter::once(crate::list_header::ALBUM_SEARCH_STYLE))
                .chain(std::iter::once(crate::home::feature_stylesheet()))
                .chain(std::iter::once(crate::item_card::controls_stylesheet()))
                .chain(palette.stylesheets.iter().map(String::as_str))
                .chain(
                    self.state
                        .settings
                        .css
                        .enabled
                        .then_some(self.state.settings.css.content.as_str()),
                ),
        ) {
            Ok(sheet) => cx.set_global(
                crate::styles::NativeStyles::new(sheet, palette.clone())
                    .with_font_metrics(
                        cx.text_system().clone(),
                        gpui::font(cx.theme().font_family.clone()),
                        window.scale_factor(),
                    )
                    .with_zoom(self.state.settings.zoom)
                    .with_variables(css_variables),
            ),
            Err(error) => warnings.push(error.to_string()),
        }
        cx.set_global(crate::scroll::ScrollTheme::new(
            &palette,
            f32::from(cx.theme().font_size),
            self.state.settings.zoom,
            cx,
        ));
        self.theme_palette = palette;
        if !warnings.is_empty() {
            self.message = warnings.join("\n");
        }
        cx.refresh_windows();
        cx.notify();
    }

    pub fn selected_theme(&self, dark: bool) -> &str {
        if self.state.settings.follow_system_theme {
            if dark {
                &self.state.settings.theme_dark
            } else {
                &self.state.settings.theme_light
            }
        } else {
            &self.state.settings.theme
        }
    }

    pub fn theme_length(&self, key: &str, fallback: f32) -> f32 {
        self.theme_palette
            .app
            .get(key)
            .and_then(|value| value.trim().trim_end_matches("px").parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 10000.0)
            .unwrap_or(fallback)
    }

    pub fn theme_overlay(&self, key: &'static str) -> AnyElement {
        let value = self
            .theme_palette
            .app
            .get(key)
            .map_or("transparent", String::as_str);
        let gradient = appearance::gradient(value, &self.theme_palette).unwrap_or_else(|_| {
            let color = appearance::theme_color(value, &self.theme_palette).unwrap_or([0.; 4]);
            appearance::Gradient {
                angle: 180.,
                corner: false,
                noise: false,
                stops: vec![
                    appearance::ColorStop {
                        color,
                        position: None
                    };
                    2
                ],
            }
        });
        let signature = format!("{gradient:?}");
        let cache = self.theme_overlays.clone();
        canvas(
            move |bounds, _, _| {
                let width = (f32::from(bounds.size.width).ceil() as u32).clamp(1, 2048);
                let height = (f32::from(bounds.size.height).ceil() as u32).clamp(1, 2048);
                cache
                    .borrow_mut()
                    .get(key, &signature, &gradient, width, height)
            },
            |bounds, image, window, _| {
                let _ = window.paint_image(bounds, Corners::default(), image, 0, false);
            },
        )
        .size_full()
        .into_any_element()
    }

    pub fn appearance_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("appearance-picker")
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .children(
                i18n::languages()
                    .into_iter()
                    .enumerate()
                    .map(|(i, choice)| {
                        crate::styles::button(("appearance-choice", i), cx)
                            .label(choice)
                            .selected(choice == self.state.settings.language)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.state.settings.language = choice.into();
                                view.save();
                                cx.notify();
                            }))
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{OverlayCache, gradient_image};
    use feishin_core::appearance;

    #[test]
    fn css_background_rasters_match_chromium_at_both_display_scales() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/css-backgrounds-reference.json")).unwrap();
        for (scale, bytes) in [
            (
                1,
                include_bytes!("../assets/css-backgrounds-1x.png").as_slice(),
            ),
            (
                2,
                include_bytes!("../assets/css-backgrounds-2x.png").as_slice(),
            ),
        ] {
            let screenshot = image::load_from_memory(bytes).unwrap().to_rgba8();
            for case in reference["cases"].as_array().unwrap() {
                let value = case["value"].as_str().unwrap();
                let width = case["width"].as_u64().unwrap() as u32;
                let height = case["height"].as_u64().unwrap() as u32;
                let top = case["top"].as_u64().unwrap() as u32;
                let gradient =
                    appearance::gradient(value, &appearance::Palette::default()).unwrap();
                let raster = gradient_image(
                    &gradient,
                    width * scale,
                    height * scale,
                    width as f32,
                    height as f32,
                );
                let pixels = raster.as_bytes(0).unwrap();
                for y in 0..height * scale {
                    for x in 0..width * scale {
                        let offset = ((y * width * scale + x) * 4) as usize;
                        let actual = [
                            pixels[offset + 2],
                            pixels[offset + 1],
                            pixels[offset],
                            pixels[offset + 3],
                        ];
                        let expected = screenshot.get_pixel(x, top * scale + y).0;
                        for channel in 0..4 {
                            let (a, b, tolerance) = if channel == 3 {
                                (actual[3], expected[3], 1)
                            } else {
                                (
                                    (actual[channel] as f32 * actual[3] as f32 / 255.).round()
                                        as u8,
                                    (expected[channel] as f32 * expected[3] as f32 / 255.).round()
                                        as u8,
                                    2,
                                )
                            };
                            assert!(
                                a.abs_diff(b) <= tolerance,
                                "{scale}x {value} ({x}, {y}): {actual:?} != {expected:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn theme_gradients_match_chrome_and_cache_original_noise() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../feishin-core/assets/theme-gradient-reference.json"
        ))
        .unwrap();
        let screenshot = image::load_from_memory(include_bytes!(
            "../../feishin-core/assets/theme-gradient-reference.png"
        ))
        .unwrap()
        .to_rgba8();
        let width = reference["width"].as_u64().unwrap() as u32;
        let height = reference["height"].as_u64().unwrap() as u32;
        for (index, value) in reference["gradients"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let value = value.as_str().unwrap();
            let gradient = appearance::gradient(value, &appearance::Palette::default()).unwrap();
            let pixels = gradient.raster(width, height);
            for y in 0..height {
                for x in 0..width {
                    let actual =
                        &pixels[((y * width + x) * 4) as usize..((y * width + x) * 4 + 4) as usize];
                    let expected = screenshot.get_pixel(x, index as u32 * height + y).0;
                    for channel in 0..4 {
                        if channel < 3 && actual[3] == 0 {
                            continue;
                        }
                        assert!(
                            if channel == 3 {
                                actual[channel].abs_diff(expected[channel]) <= 1
                            } else {
                                let a = (actual[channel] as f32 * actual[3] as f32 / 255.).round()
                                    as u8;
                                let b = (expected[channel] as f32 * expected[3] as f32 / 255.)
                                    .round() as u8;
                                a.abs_diff(b) <= 2
                            },
                            "{value} ({x},{y}): {actual:?} != {expected:?}"
                        );
                    }
                }
            }
        }
        let palette = appearance::resolve("defaultDark", std::path::Path::new("/missing")).unwrap();
        let gradient = appearance::gradient(&palette.app["overlay-header"], &palette).unwrap();
        assert!(gradient.noise);
        let mut cache = OverlayCache::default();
        let a = cache.get("header", "one", &gradient, 64, 24);
        let b = cache.get("header", "one", &gradient, 64, 24);
        assert!(std::sync::Arc::ptr_eq(&a, &b));
        let c = cache.get("header", "two", &gradient, 64, 24);
        assert!(!std::sync::Arc::ptr_eq(&a, &c));
        let d = cache.get("header", "two", &gradient, 32, 24);
        assert!(!std::sync::Arc::ptr_eq(&c, &d));
        assert_eq!(cache.0.len(), 1);
        let noise = image::load_from_memory(include_bytes!("../assets/theme-noise.png"))
            .unwrap()
            .to_rgba8();
        assert_eq!(noise.dimensions(), (300, 300));
        assert!(noise.pixels().any(|pixel| pixel.0[3] > 0));
        assert!(noise.pixels().all(|pixel| pixel.0[3] <= 13));
    }
}
