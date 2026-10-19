pub use crate::theme_colors::scale as color_scale;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Palette {
    pub mode: String,
    pub extends: Option<String>,
    #[serde(alias = "stylesheetContents")]
    pub stylesheets: Vec<String>,
    pub colors: BTreeMap<String, String>,
    #[serde(skip)]
    pub invalid_color_keys: Vec<String>,
    pub app: BTreeMap<String, String>,
    #[serde(rename = "mantineOverride", alias = "mantine_override")]
    pub mantine_override: serde_json::Value,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            mode: "dark".into(),
            extends: None,
            stylesheets: vec![],
            colors: BTreeMap::new(),
            invalid_color_keys: Vec::new(),
            app: BTreeMap::new(),
            mantine_override: serde_json::json!({}),
        }
    }
}

impl Palette {
    pub fn color_warning(&self) -> Option<String> {
        (!self.invalid_color_keys.is_empty()).then(|| {
            format!(
                "Ignored invalid color value(s) for: {}",
                self.invalid_color_keys.join(", ")
            )
        })
    }
}

pub fn builtins() -> BTreeMap<String, Palette> {
    serde_json::from_str(include_str!("../assets/palettes.json")).expect("Invalid bundled palette")
}

pub fn available(directory: &Path) -> Vec<String> {
    let mut ids = builtins()
        .into_keys()
        .filter(|id| id != "defaultTheme")
        .collect::<Vec<_>>();
    if let Ok(files) = fs::read_dir(directory) {
        ids.extend(
            files
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
                .filter_map(|path| path.file_stem().map(|id| id.to_string_lossy().into_owned())),
        );
    }
    ids.sort();
    ids.dedup();
    ids
}

pub fn resolve(id: &str, directory: &Path) -> Result<Palette> {
    fn load(
        id: &str,
        directory: &Path,
        all: &BTreeMap<String, Palette>,
        seen: &mut HashSet<String>,
    ) -> Result<Palette> {
        ensure!(
            seen.len() < 10 && seen.insert(id.to_owned()),
            "Theme inheritance contains a cycle or exceeds ten levels"
        );
        ensure!(
            !id.is_empty() && !id.contains(['/', '\\']) && id != "..",
            "Invalid theme ID"
        );
        let palette = if let Some(palette) = all.get(id) {
            palette.clone()
        } else {
            let bytes = fs::read(directory.join(format!("{id}.json")))
                .context("Could not read custom theme")?;
            ensure!(bytes.len() <= 1_000_000, "Custom theme is too large");
            let mut raw: serde_json::Value =
                serde_json::from_slice(&bytes).context("Custom theme is invalid")?;
            let mut invalid_color_keys = Vec::new();
            if let Some(colors) = raw.get_mut("colors") {
                if colors.is_null() {
                    *colors = serde_json::json!({});
                }
                if let Some(colors) = colors.as_object_mut() {
                    colors.retain(|key, value| {
                        let valid = value.as_str().is_some_and(valid_custom_color);
                        if !valid {
                            invalid_color_keys.push(key.clone());
                        }
                        valid
                    });
                }
            }
            let mut palette: Palette =
                serde_json::from_value(raw).context("Custom theme is invalid")?;
            palette.invalid_color_keys = invalid_color_keys;
            ensure!(
                palette.stylesheets.len() <= 64,
                "Too many linked stylesheets"
            );
            let root = directory.canonicalize()?;
            let mut total = 0;
            palette.stylesheets = palette
                .stylesheets
                .into_iter()
                .filter_map(|value| {
                    let result = (|| -> Result<String> {
                        if value.contains('{') {
                            return Ok(value);
                        }
                        let path = directory
                            .join(&value)
                            .canonicalize()
                            .context("Could not read linked theme stylesheet")?;
                        ensure!(
                            path.starts_with(&root) && path.is_file(),
                            "Linked stylesheet is outside the themes folder"
                        );
                        ensure!(
                            path.metadata()?.len() <= 1_000_000,
                            "Linked stylesheet is too large"
                        );
                        Ok(fs::read_to_string(path)?)
                    })();
                    match result {
                        Ok(css) => {
                            total += css.len();
                            Some(css)
                        }
                        Err(error) => {
                            log::warn!("Custom theme stylesheet was skipped: {error}");
                            None
                        }
                    }
                })
                .collect();
            ensure!(total <= 4_000_000, "Custom theme stylesheets are too large");
            palette
        };
        ensure!(
            matches!(palette.mode.as_str(), "dark" | "light"),
            "Theme mode must be dark or light"
        );
        let mut result = if let Some(parent) = &palette.extends {
            load(parent, directory, all, seen)?
        } else {
            all["defaultTheme"].clone()
        };
        result.stylesheets.extend(palette.stylesheets);
        ensure!(
            result.stylesheets.iter().map(String::len).sum::<usize>() <= 4_000_000,
            "Inherited theme stylesheets are too large"
        );
        result.colors.extend(palette.colors);
        result.invalid_color_keys.extend(palette.invalid_color_keys);
        result.app.extend(palette.app);
        merge_override(&mut result.mantine_override, &palette.mantine_override);
        result.mode = palette.mode;
        result.extends = None;
        Ok(result)
    }
    load(
        match id {
            "dark" => "defaultDark",
            "light" => "defaultLight",
            other => other,
        },
        directory,
        &builtins(),
        &mut HashSet::new(),
    )
}

// The original custom-theme loader uses validate-color, not the broader CSS parser.
// Its expressions run in Rust's bounded, linear-time regex engine.
fn valid_custom_color(value: &str) -> bool {
    static PATTERNS: std::sync::OnceLock<regex::RegexSet> = std::sync::OnceLock::new();
    value.len() <= 4096
        && PATTERNS
            .get_or_init(|| {
                let patterns: Vec<String> = serde_json::from_str(include_str!(
                    "../assets/custom-theme-color-patterns.json"
                ))
                .expect("Invalid bundled custom-theme color patterns");
                regex::RegexSet::new(patterns).expect("Invalid original color validator")
            })
            .is_match(value)
}

// Merge inherited overrides without discarding sibling properties or array entries.
pub(crate) fn merge_override(target: &mut serde_json::Value, source: &serde_json::Value) {
    use serde_json::Value;
    match (target, source) {
        (Value::Object(target), Value::Object(source)) => {
            for (key, value) in source {
                merge_override(target.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (Value::Array(target), Value::Array(source)) => {
            target.resize(target.len().max(source.len()), Value::Null);
            for (target, source) in target.iter_mut().zip(source) {
                merge_override(target, source);
            }
        }
        (target, source) => *target = source.clone(),
    }
}

/// CSS sizes come from the same TypeScript defaults embedded by the build script.
pub fn css_variables(palette: &Palette) -> BTreeMap<String, String> {
    css_variables_with_settings(
        palette,
        &crate::storage::Settings {
            use_theme_accent_color: true,
            ..Default::default()
        },
    )
}

pub fn css_variables_with_settings(
    palette: &Palette,
    settings: &crate::storage::Settings,
) -> BTreeMap<String, String> {
    let defaults: BTreeMap<String, BTreeMap<String, String>> =
        serde_json::from_str(include_str!("../assets/mantine-css.json"))
            .expect("Bundled Mantine CSS defaults");
    let mut variables = crate::mantine::variables(palette, settings);
    for (section, prefix) in [
        ("fontSizes", "font-size"),
        ("lineHeights", "line-height"),
        ("spacing", "spacing"),
        ("radius", "radius"),
        ("breakpoints", "breakpoint"),
        ("shadows", "shadow"),
    ] {
        let mut values = defaults[section].clone();
        if let Some(overrides) = palette.mantine_override[section].as_object() {
            values.extend(overrides.iter().filter_map(|(key, value)| {
                let value = match value {
                    serde_json::Value::String(value) => value.clone(),
                    serde_json::Value::Number(value) => value.to_string(),
                    _ => return None,
                };
                Some((key.clone(), value))
            }));
        }
        for (key, value) in values {
            let mantine = format!("--mantine-{prefix}-{key}");
            variables.insert(format!("--theme-{prefix}-{key}"), format!("var({mantine})"));
            variables.insert(mantine, value);
        }
    }
    let app: serde_json::Value = serde_json::from_str(include_str!("../assets/mantine-app.json"))
        .expect("Bundled Mantine app defaults");
    let radius = palette
        .mantine_override
        .get("defaultRadius")
        .unwrap_or(&app["defaultRadius"]);
    let radius = radius
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| radius.to_string());
    let radius = if let Some(value) = variables.get(&format!("--mantine-radius-{radius}")) {
        value.clone()
    } else if let Ok(pixels) = radius.trim_end_matches("px").parse::<f64>() {
        format!("calc({}rem * var(--mantine-scale))", pixels / 16.)
    } else {
        radius
    };
    variables.insert("--mantine-radius-default".into(), radius);
    variables.extend(palette.colors.iter().map(|(key, value)| {
        (
            format!("--theme-colors-{key}"),
            if key == "primary" && !settings.use_theme_accent_color {
                settings.accent.clone()
            } else {
                value.clone()
            },
        )
    }));
    variables.extend(
        palette
            .app
            .iter()
            .map(|(key, value)| (format!("--theme-{key}"), value.clone())),
    );
    for (key, value) in [
        ("--mantine-font-family", "var(--theme-content-font-family)"),
        (
            "--mantine-font-family-headings",
            "var(--theme-content-font-family)",
        ),
        ("--mantine-line-height", "var(--mantine-line-height-md)"),
    ] {
        let override_value = match key {
            "--mantine-font-family" => palette.mantine_override["fontFamily"].as_str(),
            "--mantine-font-family-headings" => {
                palette.mantine_override["headings"]["fontFamily"].as_str()
            }
            _ => None,
        };
        variables.insert(key.into(), override_value.unwrap_or(value).into());
    }
    variables
}

pub fn primary(palette: &Palette, settings: &crate::storage::Settings) -> Result<[f32; 4]> {
    crate::mantine::primary(palette, settings)
}

pub fn primary_contrast(
    palette: &Palette,
    settings: &crate::storage::Settings,
) -> Result<[f32; 4]> {
    crate::mantine::contrast(palette, settings)
}

pub fn color(value: &str) -> Result<[f32; 4]> {
    ensure!(value.len() <= 4096, "CSS color is too long");
    let components = parse_color(value, 0)?;
    ensure!(
        components.iter().all(|n| n.is_finite()),
        "Color component is not finite"
    );
    Ok(components.map(|component| component.clamp(0., 1.)))
}

fn parse_color(value: &str, depth: usize) -> Result<[f32; 4]> {
    use cssparser::{ParseError, Parser, ParserInput, Token};
    ensure!(depth <= 16, "CSS color mix is too deeply nested");
    if let Ok(color) = csscolorparser::parse(value) {
        return Ok(color.clamp().to_array());
    }
    fn operand<'i>(
        input: &mut Parser<'i, '_>,
        depth: usize,
    ) -> std::result::Result<([f32; 4], Option<f32>), ParseError<'i, ()>> {
        let before = input.try_parse(Parser::expect_percentage).ok();
        input.skip_whitespace();
        let start = input.position();
        let token = input.next()?.clone();
        let rgb = matches!(&token, Token::Function(name) if name.eq_ignore_ascii_case("rgb") || name.eq_ignore_ascii_case("rgba"));
        if matches!(token, Token::Function(_)) {
            input.parse_nested_block(|body| {
                while !body.is_exhausted() {
                    body.next()?;
                }
                Ok::<_, ParseError<'_, ()>>(())
            })?;
        }
        let mut color = parse_color(input.slice_from(start), depth + 1)
            .map_err(|_| input.new_custom_error(()))?;
        // RGB functions quantize alpha before mixing; HSL/HWB retain float alpha.
        if rgb {
            color[3] = (color[3] * 255.).round() / 255.;
        }
        let weight = before.or_else(|| input.try_parse(Parser::expect_percentage).ok());
        if weight.is_some_and(|weight| !(0. ..=1.).contains(&weight)) {
            return Err(input.new_custom_error(()));
        }
        input.expect_exhausted()?;
        Ok((color, weight))
    }
    let mut source = ParserInput::new(value);
    let operands = Parser::new(&mut source)
        .parse_entirely(|input| {
            input.expect_function_matching("color-mix")?;
            input.parse_nested_block(|input| {
                input.expect_ident_matching("in")?;
                input.expect_ident_matching("srgb")?;
                input.expect_comma()?;
                input.parse_comma_separated(|input| operand(input, depth))
            })
        })
        .map_err(|_| anyhow::anyhow!("Invalid CSS color or unsupported mixing space"))?;
    let [(left, left_weight), (right, right_weight)] = operands.as_slice() else {
        bail!("CSS color mix requires two colors");
    };
    let left_weight = left_weight.unwrap_or_else(|| 1. - right_weight.unwrap_or(0.5));
    let right_weight = right_weight.unwrap_or(1. - left_weight);
    let sum = left_weight + right_weight;
    ensure!(sum > 0., "CSS color mix weights cannot both be zero");
    let left_alpha = left[3] * left_weight / sum;
    let right_alpha = right[3] * right_weight / sum;
    let alpha = left_alpha + right_alpha;
    let mut result = [0.; 4];
    if alpha > 0. {
        for channel in 0..3 {
            result[channel] = (left[channel] * left_alpha + right[channel] * right_alpha) / alpha;
        }
    }
    result[3] = alpha * sum.min(1.);
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StopPosition {
    Percent(f32),
    Pixels(f32),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorStop {
    pub color: [f32; 4],
    pub position: Option<StopPosition>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub angle: f32,
    pub corner: bool,
    pub stops: Vec<ColorStop>,
    pub noise: bool,
}

fn css_parts(value: &str) -> Result<Vec<&str>> {
    let mut depth = 0usize;
    let mut start = 0;
    let mut parts = Vec::new();
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth
                    .checked_sub(1)
                    .context("Invalid background parentheses")?
            }
            ',' if depth == 0 => {
                parts.push(value[start..index].trim());
                start = index + 1;
            }
            _ => (),
        }
    }
    ensure!(depth == 0, "Invalid background parentheses");
    parts.push(value[start..].trim());
    Ok(parts)
}

pub fn theme_color(value: &str, palette: &Palette) -> Result<[f32; 4]> {
    fn resolve(value: &str, palette: &Palette, depth: usize) -> Result<[f32; 4]> {
        ensure!(depth < 10, "Theme color variable contains a cycle");
        if let Some(body) = value
            .trim()
            .strip_prefix("var(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let parts = css_parts(body)?;
            ensure!(parts.len() <= 2, "Invalid theme color variable");
            let key = parts[0]
                .strip_prefix("--theme-colors-")
                .context("Unknown theme color variable")?;
            let value = palette
                .colors
                .get(key)
                .map(String::as_str)
                .or_else(|| parts.get(1).copied())
                .context("Unknown theme color variable")?;
            resolve(value, palette, depth + 1)
        } else {
            color(value)
        }
    }
    resolve(value, palette, 0)
}

pub fn gradient(value: &str, palette: &Palette) -> Result<Gradient> {
    ensure!(value.len() <= 65536, "Theme gradient is too long");
    let layers = css_parts(value)?;
    let noise = layers.get(1).is_some_and(|layer| {
        *layer == "var(--theme-background-noise)"
            || crate::styles::variables(layer, &BTreeMap::new())
                == crate::styles::variables(crate::mantine::background_noise(), &BTreeMap::new())
    });
    ensure!(
        layers.len() <= 2 && (layers.len() == 1 || noise),
        "Unsupported background layer"
    );
    let body = layers[0]
        .strip_prefix("linear-gradient(")
        .and_then(|s| s.strip_suffix(')'))
        .context("Use a linear gradient")?;
    let mut parts = css_parts(body)?;
    let mut angle = 180.0f32;
    let mut corner = false;
    if parts[0].starts_with("to ") {
        angle = match parts.remove(0) {
            "to top" => 0.,
            "to right" => 90.,
            "to bottom" => 180.,
            "to left" => 270.,
            "to top right" | "to right top" => {
                corner = true;
                45.
            }
            "to bottom right" | "to right bottom" => {
                corner = true;
                135.
            }
            "to bottom left" | "to left bottom" => {
                corner = true;
                225.
            }
            "to top left" | "to left top" => {
                corner = true;
                315.
            }
            _ => bail!("Invalid gradient direction"),
        };
    } else {
        for (unit, factor) in [
            ("deg", 1.),
            ("turn", 360.),
            ("grad", 0.9),
            ("rad", 180. / std::f32::consts::PI),
        ] {
            if let Some(number) = parts[0].strip_suffix(unit) {
                angle = number.trim().parse::<f32>()? * factor;
                parts.remove(0);
                break;
            }
        }
    }
    ensure!(
        angle.is_finite() && (2..=256).contains(&parts.len()),
        "Invalid theme gradient"
    );
    let mut stops = Vec::new();
    for stop in parts {
        let mut value = stop;
        let mut positions = vec![];
        for _ in 0..2 {
            let Some((color, token)) = value.rsplit_once(char::is_whitespace) else {
                break;
            };
            let token = token.trim();
            let position = if let Some(number) = token.strip_suffix('%') {
                Some(StopPosition::Percent(number.parse::<f32>()? / 100.))
            } else if let Some(number) = token.strip_suffix("px") {
                Some(StopPosition::Pixels(number.parse::<f32>()?))
            } else if token == "0" {
                Some(StopPosition::Pixels(0.))
            } else {
                None
            };
            if let Some(position) = position {
                let number = match position {
                    StopPosition::Percent(n) | StopPosition::Pixels(n) => n,
                };
                ensure!(
                    number.is_finite() && number.abs() <= 100000.,
                    "Invalid gradient stop position"
                );
                positions.insert(0, position);
                value = color.trim_end();
            } else {
                break;
            }
        }
        let color = theme_color(value.trim(), palette)?;
        if positions.is_empty() {
            stops.push(ColorStop {
                color,
                position: None,
            });
        } else {
            stops.extend(positions.into_iter().map(|position| ColorStop {
                color,
                position: Some(position),
            }));
        }
    }
    Ok(Gradient {
        angle: angle.rem_euclid(360.),
        corner,
        stops,
        noise: layers.len() == 2,
    })
}

impl Gradient {
    /// CSS fixes decreasing explicit stops and distributes runs of omitted stops.
    pub fn resolved_stops(&self, length: f32) -> Vec<([f32; 4], f32)> {
        let last = self.stops.len() - 1;
        let mut positions = self
            .stops
            .iter()
            .map(|stop| {
                stop.position.map(|p| match p {
                    StopPosition::Percent(n) => n,
                    StopPosition::Pixels(n) => n / length.max(f32::EPSILON),
                })
            })
            .collect::<Vec<_>>();
        positions[0].get_or_insert(0.);
        positions[last].get_or_insert(1.);
        let mut previous = positions[0].unwrap();
        for position in positions.iter_mut().flatten() {
            *position = position.max(previous);
            previous = *position;
        }
        let mut left = 0;
        while left < last {
            let right = (left + 1..=last).find(|&i| positions[i].is_some()).unwrap();
            let start = positions[left].unwrap();
            let end = positions[right].unwrap();
            for (index, value) in positions.iter_mut().enumerate().take(right).skip(left + 1) {
                *value =
                    Some(start + (end - start) * (index - left) as f32 / (right - left) as f32);
            }
            left = right;
        }
        self.stops
            .iter()
            .zip(positions)
            .map(|(stop, position)| (stop.color, position.unwrap()))
            .collect()
    }

    /// Render in CSS pixels with premultiplied sRGB interpolation, including magic corners.
    pub fn raster(&self, width: u32, height: u32) -> Vec<u8> {
        let width = width.clamp(1, 2048);
        let height = height.clamp(1, 2048);
        self.raster_in_bounds(width, height, width as f32, height as f32)
    }

    /// Rasterize at display resolution while resolving pixel stops in CSS pixels.
    pub fn raster_in_bounds(
        &self,
        width: u32,
        height: u32,
        css_width: f32,
        css_height: f32,
    ) -> Vec<u8> {
        let width = width.clamp(1, 2048);
        let height = height.clamp(1, 2048);
        let (w, h) = (css_width.max(f32::EPSILON), css_height.max(f32::EPSILON));
        let radians = self.angle.to_radians();
        let (dx, dy) = if self.corner {
            (h.copysign(radians.sin()), -w.copysign(radians.cos()))
        } else {
            (radians.sin(), -radians.cos())
        };
        let magnitude = dx.hypot(dy);
        let (dx, dy) = (dx / magnitude, dy / magnitude);
        let length = (w * dx.abs() + h * dy.abs()).max(f32::EPSILON);
        let stops = self.resolved_stops(length);
        let tx = dx * w / (length * width as f32);
        let ty = dy * h / (length * height as f32);
        let offset = 0.5 - (w * 0.5 * dx + h * 0.5 * dy) / length;
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let t = (x as f32 + 0.5) * tx + (y as f32 + 0.5) * ty + offset;
                let index = stops.partition_point(|(_, position)| *position < t);
                let color = if index == 0 {
                    stops[0].0
                } else if index == stops.len() {
                    stops[stops.len() - 1].0
                } else {
                    let (left, a) = stops[index - 1];
                    let (right, b) = stops[index];
                    let mix = ((t - a) / (b - a)).clamp(0., 1.);
                    let alpha = left[3] * (1. - mix) + right[3] * mix;
                    let mut color = [0.; 4];
                    color[3] = alpha;
                    if alpha > 0. {
                        for i in 0..3 {
                            color[i] = (left[i] * left[3] * (1. - mix) + right[i] * right[3] * mix)
                                / alpha;
                        }
                    }
                    color
                };
                pixels.extend(color.map(|n| (n.clamp(0., 1.) * 255.).round() as u8));
            }
        }
        pixels
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn custom_theme_invalid_colors_match_original_sanitizing_and_inheritance() {
        use super::*;
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/custom-theme-colors-reference.json"))
                .unwrap();
        let directory = std::env::temp_dir().join(format!(
            "feishin-custom-color-validation-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).unwrap();
        let defaults = resolve("defaultTheme", &directory).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            fs::write(
                directory.join("case.json"),
                serde_json::to_vec(&serde_json::json!({"colors": {"primary": case["value"]}}))
                    .unwrap(),
            )
            .unwrap();
            let palette = resolve("case", &directory).unwrap();
            let valid = case["valid"].as_bool().unwrap();
            assert_eq!(palette.invalid_color_keys.is_empty(), valid, "{case}");
            assert_eq!(
                palette.colors["primary"],
                if valid {
                    case["value"].as_str().unwrap()
                } else {
                    &defaults.colors["primary"]
                },
                "{case}"
            );
        }
        for (id, theme) in reference["themes"].as_object().unwrap() {
            fs::write(
                directory.join(format!("{id}.json")),
                serde_json::to_vec(theme).unwrap(),
            )
            .unwrap();
        }
        let inherited = resolve("child", &directory).unwrap();
        assert_eq!(
            serde_json::json!(inherited.invalid_color_keys),
            reference["invalidKeys"]
        );
        for (key, value) in reference["colors"].as_object().unwrap() {
            assert_eq!(inherited.colors[key], value.as_str().unwrap());
        }
        assert_eq!(
            inherited.color_warning().unwrap(),
            "Ignored invalid color value(s) for: foreground, primary, surface"
        );
        fs::write(directory.join("empty.json"), r#"{"colors":null}"#).unwrap();
        assert!(
            resolve("empty", &directory)
                .unwrap()
                .color_warning()
                .is_none()
        );
        assert!(!valid_custom_color(&format!("rgb({})", "1".repeat(4096))));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn srgb_mixes_match_original_alpha_weights_nesting_and_rejection() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/css-color-mix-reference.json")).unwrap();
        for case in cases {
            let actual = super::color(case["value"].as_str().unwrap());
            if case["valid"] == false {
                assert!(actual.is_err(), "{case}");
                continue;
            }
            let actual = actual.unwrap_or_else(|error| panic!("{case}: {error}"));
            let expected = super::color(case["color"].as_str().unwrap()).unwrap();
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert!(
                    (actual - expected).abs() < 0.00001,
                    "{case}: {actual} != {expected}"
                );
            }
        }
        let mut nested = "red".to_owned();
        for _ in 0..18 {
            nested = format!("color-mix(in srgb, {nested}, blue)");
        }
        assert!(super::color(&nested).is_err());
    }

    use super::*;
    #[test]
    fn css_colors_match_independent_browser_pixels() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/css-color-reference.json")).unwrap();
        for entry in reference["colors"].as_array().unwrap() {
            let value = entry["value"].as_str().unwrap();
            let actual = color(value).unwrap().map(|n| (n * 255.).round() as u8);
            let expected = entry["rgba"].as_array().unwrap();
            for index in 0..4 {
                if index < 3 && actual[3] == 0 {
                    continue;
                }
                assert!(
                    actual[index].abs_diff(expected[index].as_u64().unwrap() as u8) <= 2,
                    "{value}: {actual:?} != {expected:?}"
                );
            }
        }
        assert!(color("var(--secret)").is_err());
        assert!(color("garbage").is_err());
        let mut palette = Palette::default();
        palette
            .colors
            .insert("test".into(), "var(--theme-colors-test)".into());
        assert!(theme_color("var(--theme-colors-test)", &palette).is_err());
        assert_eq!(
            theme_color("var(--theme-colors-missing, rebeccapurple)", &palette).unwrap(),
            color("rebeccapurple").unwrap()
        );
    }

    #[test]
    fn imports_existing_palettes_and_validates_custom_inheritance() {
        assert!(available(Path::new("/missing-directory")).len() >= 32);
        let palettes = builtins();
        let light = resolve("defaultLight", Path::new("/missing-directory")).unwrap();
        let mut settings = crate::storage::Settings::default();
        assert_eq!(
            primary(&light, &settings).unwrap(),
            crate::theme_colors::scale(color(&settings.accent).unwrap())[4]
        );
        settings.use_theme_primary_shade = false;
        settings.primary_shade = 9;
        assert_eq!(
            primary(&light, &settings).unwrap(),
            crate::theme_colors::scale(color(&settings.accent).unwrap())[9]
        );
        assert_eq!(palettes["defaultTheme"].app["content-max-width"], "1800px");
        assert_eq!(palettes["defaultTheme"].app["root-font-size"], "16px");
        assert!(palettes["defaultTheme"].app["overlay-header"].starts_with("linear-gradient("));
        assert_eq!(
            palettes["defaultLight"].app["scrollbar-handle-background"],
            "rgba(140, 140, 140, 30%)"
        );
        for id in palettes.keys() {
            let palette = resolve(id, Path::new("/missing-directory")).unwrap();
            for key in ["overlay-header", "overlay-subheader"] {
                let result = gradient(&palette.app[key], &palette).unwrap();
                assert_eq!(result.angle, 180.);
                assert_eq!(
                    result
                        .resolved_stops(1.)
                        .iter()
                        .map(|(_, n)| *n)
                        .collect::<Vec<_>>(),
                    [0., 1.]
                );
            }
        }
        assert!(palettes["glassyDark"].stylesheets[0].contains("backdrop-filter: blur(2rem)"));
        let defaults = &palettes["defaultTheme"];
        assert_eq!(
            gradient(
                "linear-gradient(to right, #123 25%, rgba(255, 255, 255, 50%) 75%)",
                defaults
            )
            .unwrap()
            .resolved_stops(1.)
            .iter()
            .map(|(_, n)| *n)
            .collect::<Vec<_>>(),
            [0.25, 0.75]
        );
        assert!(gradient("linear-gradient(NaNdeg, #fff, #000)", defaults).is_err());
        assert_eq!(
            gradient("linear-gradient(#fff 75%, #000 25%)", defaults)
                .unwrap()
                .resolved_stops(1.)
                .iter()
                .map(|(_, n)| *n)
                .collect::<Vec<_>>(),
            [0.75, 0.75]
        );
        for (_, palette) in builtins() {
            for (_, value) in palette.colors {
                color(&value).unwrap();
            }
        }
        assert_eq!(
            color("#abc").unwrap(),
            [170.0 / 255.0, 187.0 / 255.0, 204.0 / 255.0, 1.0]
        );
        assert_eq!(color("rgba(255, 0, 128, 50%)").unwrap()[3], 0.5);
        assert_eq!(color("red").unwrap(), [1., 0., 0., 1.]);
        assert_eq!(color("rgb(500,-50,0 / 200%)").unwrap(), [1., 0., 0., 1.]);
        assert!(color("rgb(NaN,0,0)").is_err());
        let dir = std::env::temp_dir().join(format!("feishin-themes-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("custom.json"),
            r##"{"extends":"nord","colors":{"primary":"#abcdef"}}"##,
        )
        .unwrap();
        let custom = resolve("custom", &dir).unwrap();
        assert_eq!(custom.colors["primary"], "#abcdef");
        assert_eq!(custom.colors["background"], "rgb(46, 52, 64)");
        fs::write(dir.join("custom-light.json"), r#"{"extends":"defaultLight","mode":"light","mantineOverride":{"primaryShade":{"dark":8}}}"#).unwrap();
        let custom = resolve("custom-light", &dir).unwrap();
        assert_eq!(custom.mantine_override["primaryShade"]["light"], 4);
        assert_eq!(custom.mantine_override["primaryShade"]["dark"], 8);
        fs::write(dir.join("sizes-parent.json"), r##"{"mantineOverride":{"spacing":{"sm":"2rem","extra":"12px"},"radius":{"md":"7px"},"colors":{"custom":["#111111","#222222"]}}}"##).unwrap();
        fs::write(dir.join("sizes-child.json"), r##"{"extends":"sizes-parent","app":{"content-max-width":"1234px"},"mantineOverride":{"spacing":{"extra":"20px"},"fontSizes":{"sm":"15px"},"colors":{"custom":["#333333"]}}}"##).unwrap();
        let custom = resolve("sizes-child", &dir).unwrap();
        assert_eq!(
            custom.mantine_override["colors"]["custom"],
            serde_json::json!(["#333333", "#222222"])
        );
        let variables = css_variables(&custom);
        let resolve_variable =
            |name: &str| crate::styles::variables(&format!("var({name})"), &variables).unwrap();
        assert_eq!(resolve_variable("--theme-spacing-sm"), "2rem");
        assert_eq!(resolve_variable("--theme-spacing-xs"), "calc(0.25rem * 1)");
        assert_eq!(resolve_variable("--theme-spacing-extra"), "20px");
        assert_eq!(resolve_variable("--theme-font-size-sm"), "15px");
        assert_eq!(
            resolve_variable("--theme-font-size-md"),
            "calc(0.875rem * 1)"
        );
        assert_eq!(resolve_variable("--theme-radius-md"), "7px");
        assert_eq!(
            resolve_variable("--theme-line-height-md"),
            "calc(1.125rem * 1)"
        );
        assert_eq!(resolve_variable("--theme-breakpoint-xs"), "36em");
        assert!(resolve_variable("--theme-shadow-md").contains("0 4px 6px"));
        assert_eq!(resolve_variable("--theme-content-max-width"), "1234px");
        fs::write(
            dir.join("linked.css"),
            ".fs-player-bar-module-container { background: red; }",
        )
        .unwrap();
        fs::write(
            dir.join("css-parent.json"),
            r#"{"extends":"glassyDark","stylesheets":["linked.css"]}"#,
        )
        .unwrap();
        fs::write(dir.join("css-child.json"), r#"{"extends":"css-parent","stylesheets":["missing.css","../../outside.css",".fs-image-module-image {border-radius:10px}"]}"#).unwrap();
        let linked = resolve("css-child", &dir).unwrap();
        assert_eq!(linked.stylesheets.len(), 3);
        assert!(linked.stylesheets[1].contains("background: red"));
        fs::write(
            dir.join("linked.css"),
            ".fs-player-bar-module-container { background: blue; }",
        )
        .unwrap();
        assert!(resolve("css-child", &dir).unwrap().stylesheets[1].contains("background: blue"));
        fs::write(dir.join("cycle.json"), r#"{"extends":"cycle"}"#).unwrap();
        assert!(resolve("cycle", &dir).is_err());
        assert!(resolve("../custom", &dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
