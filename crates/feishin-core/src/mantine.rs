use crate::{
    appearance::{self, Palette},
    storage::Settings,
};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

fn defaults() -> &'static Value {
    static DEFAULTS: OnceLock<Value> = OnceLock::new();
    DEFAULTS.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/mantine-defaults.json"))
            .expect("Bundled Mantine defaults")
    })
}

fn app_defaults() -> &'static Value {
    static DEFAULTS: OnceLock<Value> = OnceLock::new();
    DEFAULTS.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/mantine-app.json"))
            .expect("Original application CSS defaults")
    })
}

pub(crate) fn background_noise() -> &'static str {
    app_defaults()["root"]["--theme-background-noise"]
        .as_str()
        .expect("Original theme noise URL")
}

fn selected_accent<'a>(palette: &'a Palette, settings: &'a Settings) -> &'a str {
    if settings.use_theme_accent_color {
        palette
            .colors
            .get("primary")
            .or_else(|| palette.colors.get("state-info"))
            .unwrap_or(&settings.accent)
    } else {
        &settings.accent
    }
}

/// The original remote resolver uses generated colors and only the palette's
/// shade override, independently of Mantine's application defaults.
pub(crate) fn remote_primary(palette: &Palette, settings: &Settings) -> Result<String> {
    let index = if settings.use_theme_primary_shade {
        shade(&palette.mantine_override, &palette.mode)
    } else {
        usize::from(settings.primary_shade.min(9))
    };
    let [r, g, b, _] =
        crate::theme_colors::scale(appearance::color(selected_accent(palette, settings))?)[index];
    Ok(format!(
        "#{:02x}{:02x}{:02x}",
        (r * 255.).round() as u8,
        (g * 255.).round() as u8,
        (b * 255.).round() as u8
    ))
}

fn theme(palette: &Palette, settings: &Settings) -> Result<Value> {
    let mut theme = defaults()["theme"].clone();
    let app = app_defaults();
    for key in ["primaryColor", "primaryShade", "autoContrast"] {
        theme[key] = app[key].clone();
    }
    theme["colors"]["dark"] = app["colors"]["dark"].clone();
    for name in ["black", "white"] {
        if let Some(color) = palette.colors.get(name) {
            theme[name] = color.clone().into();
        }
    }
    let primary = crate::theme_colors::scale(appearance::color(selected_accent(
        palette, settings,
    ))?)
    .map(|[r, g, b, _]| {
        format!(
            "#{:02x}{:02x}{:02x}",
            (r * 255.).round() as u8,
            (g * 255.).round() as u8,
            (b * 255.).round() as u8
        )
    });
    theme["colors"]["primary"] = json!(primary);
    appearance::merge_override(&mut theme, &palette.mantine_override);
    if !settings.use_theme_primary_shade {
        theme["primaryShade"] =
            json!({"dark":settings.primary_shade.min(9),"light":settings.primary_shade.min(9)});
    }
    Ok(theme)
}

fn shade(theme: &Value, mode: &str) -> usize {
    theme["primaryShade"]
        .as_u64()
        .or_else(|| theme["primaryShade"][mode].as_u64())
        .unwrap_or(6)
        .min(9) as usize
}

fn primary_value<'a>(theme: &'a Value, mode: &str) -> Result<&'a str> {
    let name = theme["primaryColor"]
        .as_str()
        .context("Invalid primary color name")?;
    theme["colors"][name][shade(theme, mode)]
        .as_str()
        .context("Missing primary color shade")
}

fn light(color: &str, threshold: f64) -> bool {
    if let Some(rest) = color.strip_prefix("oklch(") {
        return rest
            .split_once('%')
            .and_then(|(value, _)| value.trim().parse::<f64>().ok())
            .is_some_and(|value| value / 100. > threshold);
    }
    let Ok([r, g, b, _]) = appearance::color(color) else {
        return false;
    };
    let gamma = |channel: f32| {
        let channel = f64::from(channel);
        if channel <= 0.03928 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * gamma(r) + 0.7152 * gamma(g) + 0.0722 * gamma(b) > threshold
}

fn contrast_name(theme: &Value, mode: &str) -> &'static str {
    if theme["autoContrast"].as_bool().unwrap_or(false)
        && primary_value(theme, mode)
            .is_ok_and(|color| light(color, theme["luminanceThreshold"].as_f64().unwrap_or(0.3)))
    {
        "black"
    } else {
        "white"
    }
}

pub(crate) fn primary(palette: &Palette, settings: &Settings) -> Result<[f32; 4]> {
    appearance::color(primary_value(&theme(palette, settings)?, &palette.mode)?)
}

pub(crate) fn contrast(palette: &Palette, settings: &Settings) -> Result<[f32; 4]> {
    let theme = theme(palette, settings)?;
    appearance::color(
        theme[contrast_name(&theme, &palette.mode)]
            .as_str()
            .context("Invalid contrast color")?,
    )
}

fn rgba(value: &str, alpha: Option<f64>, darken: f64) -> String {
    if value.starts_with("var(") {
        return if let Some(alpha) = alpha {
            format!(
                "color-mix(in srgb, {value}, transparent {}%)",
                (1. - alpha) * 100.
            )
        } else {
            format!("color-mix(in srgb, {value}, black {}%)", darken * 100.)
        };
    }
    let Ok([r, g, b, a]) = appearance::color(value) else {
        return value.into();
    };
    // Mantine multiplies integer sRGB channels using JavaScript doubles.
    let channel = |c: f32| (f64::from((c * 255.).round()) * (1. - darken)).round() as u8;
    format!(
        "rgba({}, {}, {}, {})",
        channel(r),
        channel(g),
        channel(b),
        alpha
            .map(|alpha| alpha.to_string())
            .unwrap_or_else(|| a.to_string())
    )
}

pub(crate) fn variables(palette: &Palette, settings: &Settings) -> BTreeMap<String, String> {
    let mode = if palette.mode == "light" {
        "light"
    } else {
        "dark"
    };
    let mut variables = BTreeMap::new();
    for map in [&defaults()["css"]["variables"], &defaults()["css"][mode]] {
        for (key, value) in map.as_object().unwrap() {
            let value = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            variables.insert(key.clone(), value);
        }
    }
    let theme = theme(palette, settings).unwrap_or_else(|_| defaults()["theme"].clone());
    for (key, property) in [
        ("white", "white"),
        ("black", "black"),
        ("font-family", "fontFamily"),
        ("font-family-monospace", "fontFamilyMonospace"),
        ("cursor-type", "cursorType"),
        ("scale", "scale"),
    ] {
        let name = if matches!(key, "white" | "black") {
            format!("--mantine-color-{key}")
        } else {
            format!("--mantine-{key}")
        };
        let value = &theme[property];
        variables.insert(
            name,
            value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string()),
        );
    }
    let primary = theme["primaryColor"].as_str().unwrap_or("primary");
    let selected = shade(&theme, mode);
    for (name, colors) in theme["colors"].as_object().unwrap() {
        let Some(colors) = colors.as_array() else {
            continue;
        };
        for (index, color) in colors.iter().enumerate() {
            if let Some(color) = color.as_str() {
                variables.insert(format!("--mantine-color-{name}-{index}"), color.into());
            }
        }
        if colors.len() < 10 {
            continue;
        }
        let var = |shade: usize| format!("var(--mantine-color-{name}-{shade})");
        for (property, value) in [
            (
                "text",
                if mode == "light" {
                    format!("var(--mantine-color-{name}-filled)")
                } else {
                    var(4)
                },
            ),
            ("filled", var(selected)),
            (
                "filled-hover",
                var(if selected == 9 { 8 } else { selected + 1 }),
            ),
            (
                "light",
                if mode == "light" {
                    var(1)
                } else {
                    rgba(colors[9].as_str().unwrap_or("black"), None, 0.5)
                },
            ),
            (
                "light-hover",
                if mode == "light" {
                    var(2)
                } else {
                    rgba(colors[9].as_str().unwrap_or("black"), None, 0.3)
                },
            ),
            ("light-color", var(if mode == "light" { 9 } else { 0 })),
            (
                "outline",
                var(if mode == "light" {
                    selected
                } else {
                    selected.saturating_sub(4)
                }),
            ),
            (
                "outline-hover",
                rgba(
                    colors[if mode == "light" {
                        selected
                    } else {
                        selected.saturating_sub(4)
                    }]
                    .as_str()
                    .unwrap_or("black"),
                    Some(0.05),
                    0.,
                ),
            ),
        ] {
            variables.insert(format!("--mantine-color-{name}-{property}"), value);
        }
    }
    for index in 0..10 {
        variables.insert(
            format!("--mantine-primary-color-{index}"),
            format!("var(--mantine-color-{primary}-{index})"),
        );
    }
    for property in [
        "filled",
        "filled-hover",
        "light",
        "light-hover",
        "light-color",
    ] {
        variables.insert(
            format!("--mantine-primary-color-{property}"),
            format!("var(--mantine-color-{primary}-{property})"),
        );
    }
    variables.insert(
        "--mantine-primary-color-contrast".into(),
        format!("var(--mantine-color-{})", contrast_name(&theme, mode)),
    );
    variables.insert(
        "--mantine-color-anchor".into(),
        format!(
            "var(--mantine-color-{primary}-{})",
            if mode == "dark" { 4 } else { selected }
        ),
    );
    if mode == "light" {
        variables.insert(
            "--mantine-color-text".into(),
            theme["black"].as_str().unwrap_or("#000").into(),
        );
        variables.insert(
            "--mantine-color-body".into(),
            theme["white"].as_str().unwrap_or("#fff").into(),
        );
    }
    for (key, value) in app_defaults()["root"].as_object().unwrap() {
        variables.insert(key.clone(), value.as_str().unwrap().into());
    }
    variables.insert(
        "--theme-colors-border".into(),
        if mode == "light" {
            "rgb(0 0 0 / 5%)"
        } else {
            "rgb(255 255 255 / 10%)"
        }
        .into(),
    );
    for (name, alpha) in [("hover", 3), ("selected", 6)] {
        let channel = if mode == "light" {
            "0 0 0"
        } else {
            "255 255 255"
        };
        variables.insert(
            format!("--theme-colors-row-{name}"),
            format!("rgb({channel} / {alpha}%)"),
        );
    }
    variables.insert(
        "--theme-colors-primary".into(),
        selected_accent(palette, settings).into(),
    );
    variables
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn color_variables_and_contrast_match_original_mantine_for_accents_modes_and_overrides() {
        let mut json = String::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../assets/mantine-variables-reference.json.gz").as_slice(),
        )
        .read_to_string(&mut json)
        .unwrap();
        let reference: Value = serde_json::from_str(&json).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let palette: Palette = serde_json::from_value(case["palette"].clone()).unwrap();
            let settings: Settings = serde_json::from_value(case["settings"].clone()).unwrap();
            let actual = appearance::css_variables_with_settings(&palette, &settings);
            for (key, value) in case["expected"].as_object().unwrap() {
                assert_eq!(
                    actual.get(key).map(String::as_str),
                    value.as_str(),
                    "{key}: mode={} settings={} overrides={}",
                    palette.mode,
                    case["settings"],
                    palette.mantine_override
                );
            }
            let filled =
                crate::styles::variables("var(--mantine-primary-color-filled)", &actual).unwrap();
            assert_eq!(
                primary(&palette, &settings).unwrap(),
                appearance::color(&filled).unwrap()
            );
            let foreground =
                crate::styles::variables("var(--mantine-primary-color-contrast)", &actual).unwrap();
            assert_eq!(
                contrast(&palette, &settings).unwrap(),
                appearance::color(&foreground).unwrap()
            );
        }
        assert_eq!(reference["cases"].as_array().unwrap().len(), 282);
    }
}
