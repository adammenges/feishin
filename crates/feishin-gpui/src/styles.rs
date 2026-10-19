use crate::app::Feishin;
use feishin_core::{
    appearance,
    styles::{Stylesheet, Target},
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, StyledExt, button::Button, input::Input};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
};

pub struct NativeStyles {
    sheet: Stylesheet,
    palette: appearance::Palette,
    variables: BTreeMap<String, String>,
    media: feishin_core::styles::MediaEnvironment,
    zoom: f32,
    text_system: Option<std::sync::Arc<TextSystem>>,
    default_font: Font,
    display_scale: f32,
    context: Vec<Target>,
    cache: RefCell<HashMap<String, StyleRefinement>>,
    motion_cache: RefCell<HashMap<String, gpui_component::button::ButtonPaintMotion>>,
    pub warnings: Vec<String>,
}
impl Global for NativeStyles {}
impl NativeStyles {
    fn resolve_value(&self, value: &str, variables: &BTreeMap<String, String>) -> Option<String> {
        feishin_core::styles::variables(value, variables)
            .and_then(|value| feishin_core::styles::viewport_lengths(&value, self.media))
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn with_font_metrics(
        mut self,
        system: std::sync::Arc<TextSystem>,
        default_font: Font,
        display_scale: f32,
    ) -> Self {
        self.text_system = Some(system);
        self.default_font = default_font;
        self.display_scale = display_scale;
        self.cache.get_mut().clear();
        self.motion_cache.get_mut().clear();
        self
    }

    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = if zoom.is_finite() && zoom > 0. {
            zoom
        } else {
            1.
        };
        self.cache.get_mut().clear();
        self.motion_cache.get_mut().clear();
        self
    }
    pub fn with_variables(mut self, variables: BTreeMap<String, String>) -> Self {
        self.variables = variables;
        self.cache.get_mut().clear();
        self.motion_cache.get_mut().clear();
        self
    }

    pub fn new(sheet: Stylesheet, palette: appearance::Palette) -> Self {
        let mut warnings = sheet.warnings.clone();
        for property in sheet.property_names() {
            if !matches!(
                property,
                "background"
                    | "background-color"
                    | "background-image"
                    | "backdrop-filter"
                    | "filter"
                    | "color"
                    | "opacity"
                    | "border"
                    | "border-width"
                    | "border-color"
                    | "border-radius"
                    | "border-top"
                    | "border-right"
                    | "border-bottom"
                    | "border-left"
                    | "border-top-width"
                    | "border-right-width"
                    | "border-bottom-width"
                    | "border-left-width"
                    | "width"
                    | "height"
                    | "max-width"
                    | "max-height"
                    | "min-width"
                    | "min-height"
                    | "padding"
                    | "padding-top"
                    | "padding-right"
                    | "padding-bottom"
                    | "padding-left"
                    | "margin"
                    | "margin-top"
                    | "margin-right"
                    | "margin-bottom"
                    | "margin-left"
                    | "gap"
                    | "letter-spacing"
                    | "text-shadow"
                    | "font-size"
                    | "font-family"
                    | "font-weight"
                    | "font-style"
                    | "font-variant-numeric"
                    | "font-feature-settings"
                    | "font-variation-settings"
                    | "text-spacing-trim"
                    | "word-break"
                    | "overflow-wrap"
                    | "direction"
                    | "text-align"
                    | "text-decoration"
                    | "text-decoration-line"
                    | "text-decoration-color"
                    | "text-decoration-thickness"
                    | "text-decoration-style"
                    | "cursor"
                    | "display"
                    | "visibility"
                    | "overflow"
                    | "overflow-x"
                    | "overflow-y"
                    | "flex-direction"
                    | "flex-wrap"
                    | "flex-basis"
                    | "flex"
                    | "flex-grow"
                    | "flex-shrink"
                    | "align-items"
                    | "align-self"
                    | "align-content"
                    | "justify-content"
                    | "box-shadow"
                    | "position"
                    | "top"
                    | "right"
                    | "bottom"
                    | "left"
                    | "inset"
                    | "aspect-ratio"
                    | "object-fit"
                    | "line-height"
                    | "white-space"
                    | "text-overflow"
                    | "row-gap"
                    | "column-gap"
                    | "border-style"
                    | "transform"
                    | "transform-origin"
                    | "transition"
                    | "transition-property"
                    | "transition-duration"
                    | "transition-delay"
                    | "transition-timing-function"
            ) && logical_spacing_sides(property, false).is_none()
                && !property.starts_with("--")
            {
                warnings.push(format!(
                    "CSS property has no native equivalent yet: {property}"
                ));
            }
        }
        for (name, value) in sheet.property_values() {
            if name == "transform"
                && !feishin_core::styles::is_variable_value(value)
                && !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "inherit" | "initial" | "unset" | "revert" | "revert-layer"
                )
                && css_transform(value, 16.).is_none()
            {
                warnings.push(format!("CSS transform is not supported: {value}"));
            }
        }
        warnings.sort();
        warnings.dedup();
        let mut root = Target::new("body", "native-root", &[]);
        root.attributes
            .insert("data-mantine-color-scheme".into(), palette.mode.clone());
        Self {
            sheet,
            variables: appearance::css_variables(&palette),
            media: feishin_core::styles::MediaEnvironment {
                dark: palette.mode == "dark",
                ..Default::default()
            },
            palette,
            zoom: 1.,
            text_system: None,
            default_font: gpui::font("Inter"),
            display_scale: 1.,
            context: vec![root],
            cache: RefCell::default(),
            motion_cache: RefCell::default(),
            warnings,
        }
    }
    fn button_motion(&self, target: Target) -> gpui_component::button::ButtonPaintMotion {
        use feishin_core::css_transition::{Easing, Transitions};
        use gpui_component::{animation::CssTiming, button::ButtonPaintMotion};
        let mut path = self.context.clone();
        path.push(target);
        let key = format!("{path:?}:{:?}", self.media);
        if let Some(motion) = self.motion_cache.borrow().get(&key) {
            return *motion;
        }
        let mut variables = self.variables.clone();
        let mut transitions = Transitions::default();
        let mut legacy = [true; 3];
        let is_legacy = |value: &str| {
            let value = value.to_ascii_lowercase();
            !["color-mix(", "color(", "oklab(", "oklch(", "lab(", "lch("]
                .iter()
                .any(|function| value.contains(function))
        };
        for end in 1..=path.len() {
            let parent = transitions.clone();
            transitions = Transitions::default();
            let parent_legacy = legacy;
            legacy = [true, parent_legacy[2], parent_legacy[2]];
            let declarations = self
                .sheet
                .ordered_declarations_with_environment(&path[..end], self.media);
            variables = feishin_core::styles::cascade_variables(
                &variables,
                declarations
                    .iter()
                    .filter(|(name, _)| name.starts_with("--"))
                    .cloned(),
            );
            let declarations: Vec<_> = declarations
                .into_iter()
                .map(|(name, value)| {
                    let computed = self
                        .resolve_value(&value, &variables)
                        .unwrap_or_else(|| "unset".into());
                    (
                        name,
                        computed,
                        feishin_core::styles::is_variable_value(&value),
                    )
                })
                .collect();
            // currentColor on a paint property refers to the final foreground.
            for (_, value, _) in declarations.iter().filter(|(name, _, _)| name == "color") {
                if matches!(
                    value.to_ascii_lowercase().as_str(),
                    "inherit" | "unset" | "currentcolor"
                ) {
                    legacy[2] = parent_legacy[2];
                } else if color(value, &self.palette).is_some() {
                    legacy[2] = is_legacy(value);
                }
            }
            legacy[1] = legacy[2];
            for (name, value, variable) in declarations {
                if !transitions.apply(&name, &value, &parent) && variable {
                    transitions.apply(&name, "unset", &parent);
                }
                let index = match name.as_str() {
                    "background" | "background-color" => 0,
                    "border" | "border-color" | "border-top" | "border-top-color" => 1,
                    _ => continue,
                };
                if value.eq_ignore_ascii_case("inherit") {
                    legacy[index] = parent_legacy[index];
                    continue;
                }
                if value.eq_ignore_ascii_case("currentcolor") {
                    legacy[index] = legacy[2];
                    continue;
                }
                let mut paint = StyleRefinement::default();
                apply_property_value(&mut paint, &name, &value, &self.palette, 16.);
                if paint.background.is_some() || paint.border_color.is_some() {
                    legacy[index] = is_legacy(&value);
                }
            }
        }
        let motion = ButtonPaintMotion {
            timing: ["background-color", "border-top-color", "color", "opacity"].map(|property| {
                let timing = transitions.for_property(property);
                let (curve, steps) = match timing.easing {
                    Easing::Bezier(curve) => (curve, None),
                    Easing::Steps { count, start, end } => {
                        ([0., 0., 1., 1.], Some((count, start, end)))
                    }
                };
                CssTiming {
                    duration: std::time::Duration::from_secs_f32(timing.duration),
                    delay: timing.delay,
                    curve,
                    steps,
                }
            }),
            legacy_colors: legacy,
        };
        let mut cache = self.motion_cache.borrow_mut();
        if cache.len() >= 8192 {
            cache.clear();
        }
        cache.insert(key, motion);
        motion
    }
    pub(crate) fn style(
        &self,
        target: Target,
        extra: Option<Target>,
        font: f32,
    ) -> StyleRefinement {
        self.style_in(target, extra, font)
    }
    pub(crate) fn style_in(
        &self,
        mut target: Target,
        parents: impl IntoIterator<Item = Target>,
        font: f32,
    ) -> StyleRefinement {
        let font = font / self.zoom;
        let mut path = self.context.clone();
        if target.id == "native-root" {
            if let Some(root) = path.first() {
                target.classes.extend(root.classes.iter().cloned());
                target.children.extend(root.children.iter().cloned());
            }
            path.clear();
        }
        let mut parents = parents.into_iter().peekable();
        if parents
            .peek()
            .is_some_and(|parent| parent.id == "native-modal-root")
        {
            path = modal_root_context(&path);
        }
        for target in parents.chain(std::iter::once(target)) {
            if !target.id.is_empty() && path.last().is_some_and(|parent| parent.id == target.id) {
                path.pop();
            }
            path.push(target);
        }
        self.style_for_path(&path, font)
    }

    fn style_for_path(&self, path: &[Target], font: f32) -> StyleRefinement {
        let revision = self
            .text_system
            .as_ref()
            .map(|system| system.fonts_revision());
        let key = format!("{path:?}:{font}:{:?}:{revision:?}", self.media);
        if let Some(style) = self.cache.borrow().get(&key) {
            return style.clone();
        }
        let mut style = StyleRefinement::default();
        let mut typography = BTreeMap::<String, String>::new();
        let mut decorations = BTreeMap::new();
        let mut vars = self.variables.clone();
        let mut numeric = vec![("tnum".to_owned(), 1)];
        let mut features = vec![];
        let mut has_font_features = false;
        let mut variations = None;
        let mut spacing = TextSpacingTrim::Normal;
        let mut has_spacing = false;
        let mut direction = None;
        let mut font_style = None;
        let mut letter_spacing: Option<DefiniteLength> = None;
        let mut text_shadows = None;
        let mut backdrop_blur = None;
        let mut backdrop_saturation = None;
        let mut backdrop_filters = None;
        let mut foreground_filters = None;
        let mut current_color = None;
        let mut word_break = None;
        let mut overflow_wrap = None;
        for end in 1..=path.len() {
            let parent_typography = typography.clone();
            let parent_numeric = numeric.clone();
            let parent_features = features.clone();
            let parent_variations = variations.clone();
            let parent_spacing = spacing;
            let parent_direction = direction;
            match path[end - 1].attributes.get("dir").map(String::as_str) {
                Some("rtl") => direction = Some(TextDirection::Rtl),
                Some("ltr") => direction = Some(TextDirection::Ltr),
                _ => {}
            }
            let parent_font_style = font_style;
            let parent_letter_spacing = letter_spacing;
            let parent_text_shadows = text_shadows.clone();
            let parent_backdrop_blur = backdrop_blur;
            let parent_backdrop_saturation = backdrop_saturation;
            let parent_backdrop_filters = backdrop_filters.clone();
            let parent_foreground_filters = foreground_filters.clone();
            let parent_color = current_color;
            backdrop_blur = None;
            backdrop_saturation = None;
            backdrop_filters = None;
            foreground_filters = None;
            let parent_word_break = word_break;
            let parent_overflow_wrap = overflow_wrap;
            vars = feishin_core::styles::cascade_variables(
                &vars,
                self.sheet
                    .declarations_with_environment(&path[..end], self.media)
                    .into_iter()
                    .filter(|(name, _)| name.starts_with("--")),
            );
            let declarations = self
                .sheet
                .ordered_declarations_with_environment(&path[..end], self.media);
            let parent_font = parent_typography
                .get("font-size")
                .and_then(|value| length(value, font))
                .unwrap_or(font);
            let parent_weight = parent_typography
                .get("font-weight")
                .and_then(|value| value.parse::<f32>().ok())
                .unwrap_or(400.);
            // Font size must be computed before em lengths, independently of declaration order.
            for property in [
                "font-size",
                "font-weight",
                "font-family",
                "line-height",
                "text-align",
                "white-space",
            ] {
                for (name, value) in declarations.iter().filter(|(name, _)| name == property) {
                    let variable = feishin_core::styles::is_variable_value(value);
                    let value = self
                        .resolve_value(value, &vars)
                        .unwrap_or_else(|| "unset".into());
                    let value = match value.trim().to_ascii_lowercase().as_str() {
                        "inherit" | "unset" | "revert" => {
                            if let Some(value) = parent_typography.get(name) {
                                typography.insert(name.clone(), value.clone());
                            } else {
                                typography.remove(name);
                            }
                            continue;
                        }
                        "initial" => match name.as_str() {
                            "font-size" => "16px".into(),
                            "font-weight" => "400".into(),
                            "font-family" => "serif".into(),
                            "line-height" | "white-space" => "normal".into(),
                            "text-align" => "start".into(),
                            _ => unreachable!(),
                        },
                        _ => value,
                    };
                    let own_font = typography
                        .get("font-size")
                        .and_then(|value| length(value, font))
                        .unwrap_or(parent_font);
                    let computed = if name == "font-size" {
                        let size = match value.trim().to_ascii_lowercase().as_str() {
                            "xx-small" => Some(9.),
                            "x-small" => Some(10.),
                            "small" => Some(13.),
                            "medium" => Some(16.),
                            "large" => Some(18.),
                            "x-large" => Some(24.),
                            "xx-large" => Some(32.),
                            "xxx-large" => Some(48.),
                            "smaller" => Some(parent_font / 1.2),
                            "larger" => Some(parent_font * 1.2),
                            _ => {
                                if let Some(percent) = value.trim().strip_suffix('%') {
                                    percent.parse::<f32>().ok().map(|n| parent_font * n / 100.)
                                } else {
                                    feishin_core::styles::font_relative_lengths(
                                        &value,
                                        parent_font,
                                        font,
                                    )
                                    .and_then(|value| length(&value, font))
                                }
                            }
                        };
                        size.filter(|n| n.is_finite() && *n >= 0. && *n <= 10000.)
                            .map(|n| format!("{n}px"))
                    } else if name == "font-weight" {
                        Some(match value.trim().to_ascii_lowercase().as_str() {
                            "bolder" => (if parent_weight < 350. {
                                400
                            } else if parent_weight < 550. {
                                700
                            } else {
                                900
                            })
                            .to_string(),
                            "lighter" => (if parent_weight < 550. {
                                100
                            } else if parent_weight < 750. {
                                400
                            } else {
                                700
                            })
                            .to_string(),
                            "normal" => "400".into(),
                            "bold" => "700".into(),
                            _ => value,
                        })
                    } else if name == "line-height" {
                        if let Some(percent) = value.trim().strip_suffix('%') {
                            percent
                                .parse::<f32>()
                                .ok()
                                .map(|n| format!("{}px", own_font * n / 100.))
                        } else {
                            feishin_core::styles::font_relative_lengths(&value, own_font, font)
                        }
                    } else {
                        Some(value)
                    };
                    let computed = computed.filter(|value| {
                        if name == "line-height" && value.trim().eq_ignore_ascii_case("normal") {
                            return true;
                        }
                        let mut parsed = StyleRefinement::default();
                        apply_property(&mut parsed, name, value, &self.palette, font);
                        parsed
                            .text
                            .as_ref()
                            .is_some_and(|text| *text != TextStyleRefinement::default())
                    });
                    if let Some(value) = computed {
                        typography.insert(name.clone(), value);
                    } else if variable {
                        // A substituted invalid value unsets the property; it cannot revive an earlier declaration.
                        if let Some(value) = parent_typography.get(name) {
                            typography.insert(name.clone(), value.clone());
                        } else {
                            typography.remove(name);
                        }
                    }
                }
            }
            let element_font = typography
                .get("font-size")
                .and_then(|value| length(value, font))
                .unwrap_or(parent_font);
            for (name, value) in declarations {
                if inherited_typography_property(&name) {
                    continue;
                }
                let value = if matches!(
                    name.as_str(),
                    "word-break"
                        | "overflow-wrap"
                        | "backdrop-filter"
                        | "filter"
                        | "font-style"
                        | "letter-spacing"
                        | "text-shadow"
                        | "direction"
                        | "text-spacing-trim"
                        | "font-variant-numeric"
                        | "font-feature-settings"
                        | "font-variation-settings"
                ) && feishin_core::styles::is_variable_value(&value)
                {
                    self.resolve_value(&value, &vars)
                        .and_then(|value| {
                            feishin_core::styles::font_relative_lengths(&value, element_font, font)
                        })
                        .filter(|value| valid_inherited_css_value(&name, value, font))
                        .unwrap_or_else(|| "unset".into())
                } else {
                    value
                };
                let value = self
                    .resolve_value(&value, &vars)
                    .and_then(|value| {
                        feishin_core::styles::font_relative_lengths(&value, element_font, font)
                    })
                    .unwrap_or(value);
                if name == "text-shadow" {
                    if let Some(value) = self.resolve_value(&value, &vars) {
                        text_shadows = match value.trim().to_ascii_lowercase().as_str() {
                            "inherit" | "unset" | "revert" => parent_text_shadows.clone(),
                            "initial" | "none" => Some(vec![]),
                            _ => feishin_core::styles::text_shadows(&value, font)
                                .map(|shadows| {
                                    shadows
                                        .into_iter()
                                        .map(|shadow| gpui::TextShadow {
                                            offset: point(
                                                px(shadow.offset[0]),
                                                px(shadow.offset[1]),
                                            ),
                                            blur_radius: px(shadow.blur_radius),
                                            color: shadow
                                                .color
                                                .map(|[r, g, b, a]| Rgba { r, g, b, a }.into()),
                                        })
                                        .collect()
                                })
                                .or(text_shadows),
                        };
                    }
                    continue;
                }
                if name == "letter-spacing" {
                    if let Some(value) = self.resolve_value(&value, &vars) {
                        letter_spacing = match value.trim().to_ascii_lowercase().as_str() {
                            "inherit" | "unset" | "revert" => parent_letter_spacing,
                            "initial" | "normal" => Some(px(0.).into()),
                            _ => tracking(&value, font).or(letter_spacing),
                        };
                    }
                    continue;
                }
                if name == "color" {
                    if let Some(value) = self.resolve_value(&value, &vars) {
                        current_color = match value.trim().to_ascii_lowercase().as_str() {
                            "inherit" | "unset" | "revert" | "currentcolor" => parent_color,
                            "initial" => Some(black()),
                            _ => color(&value, &self.palette).or(current_color),
                        };
                    }
                    continue;
                }
                if name == "word-break" || name == "overflow-wrap" {
                    if let Some(value) = self.resolve_value(&value, &vars) {
                        let (target, parent) = if name == "word-break" {
                            (&mut word_break, parent_word_break)
                        } else {
                            (&mut overflow_wrap, parent_overflow_wrap)
                        };
                        let parsed = match value.trim().to_ascii_lowercase().as_str() {
                            "inherit" | "unset" | "revert" => parent,
                            "normal" | "initial" => Some(TextWrap::Normal),
                            "break-all" if name == "word-break" => Some(TextWrap::BreakAll),
                            "keep-all" if name == "word-break" => Some(TextWrap::KeepAll),
                            "break-word" => Some(TextWrap::BreakWord),
                            "anywhere" if name == "overflow-wrap" => Some(TextWrap::BreakAll),
                            _ => continue,
                        };
                        *target = parsed;
                    }
                    continue;
                }
                if name == "filter" {
                    if let Some(value) = self.resolve_value(&value, &vars)
                        && let Some(filter) = feishin_core::styles::backdrop_filter(&value, font)
                    {
                        use feishin_core::styles::BackdropFilter;
                        let operations = match filter {
                            BackdropFilter::Inherit => {
                                parent_foreground_filters.clone().unwrap_or_default()
                            }
                            BackdropFilter::Blur(_)
                                if matches!(
                                    value.trim().to_ascii_lowercase().as_str(),
                                    "none" | "initial" | "unset" | "revert"
                                ) =>
                            {
                                vec![]
                            }
                            BackdropFilter::Blur(sigma) => {
                                vec![gpui::BackdropFilterOperation::Blur(sigma)]
                            }
                            BackdropFilter::BlurSaturate(sigma, saturation) => vec![
                                gpui::BackdropFilterOperation::Blur(sigma),
                                gpui::BackdropFilterOperation::Saturate(saturation),
                            ],
                            BackdropFilter::Chain(operations) => operations
                                .into_iter()
                                .map(native_filter_operation)
                                .collect(),
                        };
                        foreground_filters = Some(operations);
                    }
                    continue;
                }
                if name == "backdrop-filter" {
                    if let Some(value) = self.resolve_value(&value, &vars)
                        && let Some(filter) = feishin_core::styles::backdrop_filter(&value, font)
                    {
                        let (sigma, saturation, operations) = match filter {
                            feishin_core::styles::BackdropFilter::Inherit => (
                                parent_backdrop_blur.unwrap_or(px(0.)),
                                parent_backdrop_saturation.unwrap_or(1.),
                                parent_backdrop_filters.clone().unwrap_or_default(),
                            ),
                            feishin_core::styles::BackdropFilter::Blur(sigma) => {
                                (px(sigma), 1., vec![])
                            }
                            feishin_core::styles::BackdropFilter::BlurSaturate(
                                sigma,
                                saturation,
                            ) => (px(sigma), saturation, vec![]),
                            feishin_core::styles::BackdropFilter::Chain(operations) => (
                                px(0.),
                                1.,
                                operations
                                    .into_iter()
                                    .map(native_filter_operation)
                                    .collect(),
                            ),
                        };
                        backdrop_blur = Some(sigma);
                        backdrop_saturation = Some(saturation);
                        backdrop_filters = Some(operations);
                    }
                    continue;
                }
                if name == "font-style" {
                    if let Some(value) = self.resolve_value(&value, &vars) {
                        let parsed = match feishin_core::fonts::style_keyword(&value) {
                            Some("inherit" | "unset" | "revert") => parent_font_style,
                            Some("normal" | "initial") => Some(FontStyle::Normal),
                            Some("italic") => Some(FontStyle::Italic),
                            Some("oblique") => Some(FontStyle::Oblique),
                            _ => continue,
                        };
                        font_style = parsed;
                    }
                    continue;
                }
                if name == "direction" {
                    if let Some(value) = self.resolve_value(&value, &vars) {
                        match value.trim().to_ascii_lowercase().as_str() {
                            "inherit" | "unset" | "revert" => direction = parent_direction,
                            "ltr" | "initial" => direction = Some(TextDirection::Ltr),
                            "rtl" => direction = Some(TextDirection::Rtl),
                            _ => {}
                        }
                    }
                    continue;
                }
                if name == "text-spacing-trim" {
                    if let Some(value) = self.resolve_value(&value, &vars) {
                        let parsed = match value.trim().to_ascii_lowercase().as_str() {
                            "inherit" | "unset" | "revert" => Some(parent_spacing),
                            "initial" => Some(TextSpacingTrim::Normal),
                            _ => TextSpacingTrim::parse(&value),
                        };
                        if let Some(parsed) = parsed {
                            spacing = parsed;
                            has_spacing = true;
                        }
                    }
                    continue;
                }
                if name == "font-variation-settings" {
                    let parsed = match value.trim().to_ascii_lowercase().as_str() {
                        "inherit" | "unset" | "revert" => {
                            Some(parent_variations.clone().unwrap_or_default())
                        }
                        "initial" => Some(gpui::FontVariations::default()),
                        _ => feishin_core::fonts::variation_settings(&value)
                            .and_then(|values| gpui::FontVariations::try_from(values).ok()),
                    };
                    if let Some(parsed) = parsed {
                        variations = Some(parsed);
                    }
                    continue;
                }
                let (target, inherited, parse) = match name.as_str() {
                    "font-variant-numeric" => (
                        &mut numeric,
                        &parent_numeric,
                        feishin_core::fonts::numeric_features
                            as fn(&str) -> Option<Vec<(String, u32)>>,
                    ),
                    "font-feature-settings" => (
                        &mut features,
                        &parent_features,
                        feishin_core::fonts::feature_settings
                            as fn(&str) -> Option<Vec<(String, u32)>>,
                    ),
                    _ => continue,
                };
                let Some(value) = self.resolve_value(&value, &vars) else {
                    continue;
                };
                let parsed = match value.trim().to_ascii_lowercase().as_str() {
                    "inherit" | "unset" | "revert" => Some(inherited.clone()),
                    "initial" => Some(vec![]),
                    _ => parse(&value),
                };
                if let Some(parsed) = parsed {
                    *target = parsed;
                    has_font_features = true;
                }
            }
        }
        let normal_line_height = typography
            .get("line-height")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("normal"));
        for (name, value) in typography {
            apply_property(&mut style, &name, &value, &self.palette, font);
        }
        // Chromium resolves currentColor on the element using an inherited filter.
        if let Some(color) = current_color {
            // Generated boxes may be rendered separately from their originating
            // native element (for example the slider's unfilled track).
            if path
                .last()
                .is_some_and(|target| target.pseudo_element.is_some())
            {
                style.text.get_or_insert_with(Default::default).color = Some(color);
            }
            for operations in [&mut foreground_filters, &mut backdrop_filters]
                .into_iter()
                .flatten()
            {
                for operation in operations {
                    if let gpui::BackdropFilterOperation::DropShadow {
                        color: shadow_color,
                        ..
                    } = operation
                        && shadow_color.is_none()
                    {
                        let rgba = color.to_rgb();
                        *shadow_color = Some([rgba.r, rgba.g, rgba.b, rgba.a]);
                    }
                }
            }
        }
        if let Some(spacing) = letter_spacing {
            let text = style.text.get_or_insert_with(Default::default);
            let (absolute, fraction) = match spacing {
                DefiniteLength::Absolute(value) => (value.to_pixels(px(font)), 0.),
                DefiniteLength::Fraction(fraction) => (px(0.), fraction),
                DefiniteLength::Calc(pixels, fraction) => (pixels, fraction),
            };
            text.letter_spacing = Some(absolute);
            text.letter_spacing_fraction = Some(fraction);
        }
        if let Some(shadows) = text_shadows {
            style.text.get_or_insert_with(Default::default).text_shadows = Some(shadows);
        }
        if let Some(font_style) = font_style {
            style.text.get_or_insert_with(Default::default).font_style = Some(font_style);
        }
        style.backdrop_blur = backdrop_blur;
        style.backdrop_saturation = backdrop_saturation;
        style.backdrop_filters = backdrop_filters;
        style.foreground_filters = foreground_filters;
        if let Some(direction) = direction {
            style
                .text
                .get_or_insert_with(Default::default)
                .text_direction = Some(direction);
        }
        if word_break.is_some() || overflow_wrap.is_some() {
            style.text.get_or_insert_with(Default::default).text_wrap = Some(match word_break {
                Some(TextWrap::KeepAll) => match overflow_wrap {
                    Some(TextWrap::BreakAll | TextWrap::BreakWord) => TextWrap::KeepAllBreakWord,
                    _ => TextWrap::KeepAll,
                },
                Some(TextWrap::BreakAll | TextWrap::BreakWord) => word_break.unwrap(),
                _ => overflow_wrap.unwrap_or(TextWrap::Normal),
            });
        }
        if has_spacing {
            style
                .text
                .get_or_insert_with(Default::default)
                .text_spacing_trim = Some(spacing);
        }
        if has_font_features {
            // Low-level settings override variant features regardless of CSS declaration order.
            let mut combined = numeric.into_iter().collect::<BTreeMap<_, _>>();
            combined.extend(features);
            style
                .text
                .get_or_insert_with(Default::default)
                .font_features = Some(FontFeatures(std::sync::Arc::new(
                combined.into_iter().collect(),
            )));
        }
        if let Some(variations) = variations {
            style
                .text
                .get_or_insert_with(Default::default)
                .font_variations = Some(variations);
        }
        if normal_line_height {
            let text = style.text.get_or_insert_with(Default::default);
            if let Some(system) = &self.text_system {
                let mut face = self.default_font.clone();
                if let Some(family) = &text.font_family {
                    face.family = family.clone();
                }
                if let Some(weight) = text.font_weight {
                    face.weight = weight;
                }
                if let Some(style) = text.font_style {
                    face.style = style;
                }
                if let Some(variations) = &text.font_variations {
                    face.variations = variations.clone();
                }
                let size = text
                    .font_size
                    .map(|size| size.to_pixels(px(font)))
                    .unwrap_or(px(font));
                text.line_height = Some(
                    (system.normal_line_height(&face, size * self.zoom, self.display_scale)
                        / self.zoom)
                        .into(),
                );
            } else {
                text.line_height = Some(TextStyle::default().line_height);
            }
        }
        let element_font = style
            .text
            .as_ref()
            .and_then(|text| text.font_size)
            .map(|size| f32::from(size.to_pixels(px(font))))
            .unwrap_or(font);
        let mut parent_layout = None;
        for (name, value) in self
            .sheet
            .ordered_declarations_with_environment(path, self.media)
        {
            let computed = if let Some(initial) = initial_layout_css_value(&name)
                && feishin_core::styles::is_variable_value(&value)
            {
                Some(
                    self.resolve_value(&value, &vars)
                        .filter(|value| valid_layout_css_value(&name, value, &self.palette, font))
                        .unwrap_or_else(|| initial.into()),
                )
            } else {
                self.resolve_value(&value, &vars)
            };
            if let Some(value) = computed.and_then(|value| {
                feishin_core::styles::font_relative_lengths(&value, element_font, font)
            }) {
                if matches!(
                    name.as_str(),
                    "font-style" | "letter-spacing" | "text-shadow" | "backdrop-filter" | "filter"
                ) || inherited_typography_property(&name)
                {
                    continue;
                } else if name == "text-decoration" {
                    if let Some(properties) = decoration_shorthand(&value, &self.palette, font) {
                        decorations.extend(properties);
                    }
                } else if name.starts_with("text-decoration-") {
                    if valid_decoration_longhand(&name, &value, &self.palette, font) {
                        decorations.insert(name, value);
                    }
                } else if value.trim().eq_ignore_ascii_case("inherit")
                    && (spacing_indices(&name, false).is_some()
                        || matches!(
                            name.as_str(),
                            "transform" | "transform-origin" | "object-fit"
                        ))
                {
                    let parent = parent_layout.get_or_insert_with(|| {
                        let mut parent = if path.len() > 1 {
                            self.style_for_path(&path[..path.len() - 1], font)
                        } else {
                            StyleRefinement::default()
                        };
                        scale_css_lengths(&mut parent, 1. / self.zoom);
                        parent
                    });
                    match name.as_str() {
                        "object-fit" => {
                            style.object_fit =
                                Some(parent.object_fit.unwrap_or(gpui::ObjectFit::Fill));
                        }
                        "transform" => {
                            style.paint_scale = Some(parent.paint_scale.unwrap_or(1.));
                            style.paint_translation =
                                Some(parent.paint_translation.unwrap_or_default());
                            style.paint_translation_fraction =
                                Some(parent.paint_translation_fraction.unwrap_or_default());
                        }
                        "transform-origin" => {
                            style.paint_scale_origin =
                                Some(parent.paint_scale_origin.unwrap_or(point(0.5, 0.5)));
                            style.paint_scale_origin_offset =
                                Some(parent.paint_scale_origin_offset.unwrap_or_default());
                        }
                        _ => inherit_spacing(&mut style, parent, &name),
                    }
                } else {
                    apply_property(&mut style, &name, &value, &self.palette, font);
                }
            }
        }
        // Decoration longhands are computed independently of declaration order.
        for name in [
            "text-decoration-line",
            "text-decoration-color",
            "text-decoration-style",
            "text-decoration-thickness",
        ] {
            if let Some(value) = decorations.get(name) {
                apply_property(&mut style, name, value, &self.palette, font);
            }
        }
        scale_css_lengths(&mut style, self.zoom);
        // Chromium snaps borders to device pixels before laying out their children.
        for width in [
            &mut style.border_widths.top,
            &mut style.border_widths.right,
            &mut style.border_widths.bottom,
            &mut style.border_widths.left,
        ]
        .into_iter()
        .flatten()
        {
            *width =
                snapped_border_width(width.to_pixels(px(font * self.zoom)), self.display_scale)
                    .into();
        }
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= 8192 {
            cache.clear();
        }
        cache.insert(key, style.clone());
        style
    }
}
fn inherited_typography_property(name: &str) -> bool {
    matches!(
        name,
        "font-size" | "font-weight" | "font-family" | "line-height" | "text-align" | "white-space"
    )
}

fn scale_css_lengths(style: &mut StyleRefinement, zoom: f32) {
    if zoom == 1. {
        return;
    }
    fn absolute(value: &mut AbsoluteLength, zoom: f32) {
        if let AbsoluteLength::Pixels(value) = value {
            *value *= zoom;
        }
    }
    fn definite(value: &mut DefiniteLength, zoom: f32) {
        match value {
            DefiniteLength::Absolute(value) => absolute(value, zoom),
            DefiniteLength::Calc(pixels, _) => *pixels *= zoom,
            DefiniteLength::Fraction(_) => {}
        }
    }
    for value in [
        &mut style.inset.top,
        &mut style.inset.right,
        &mut style.inset.bottom,
        &mut style.inset.left,
        &mut style.size.width,
        &mut style.size.height,
        &mut style.min_size.width,
        &mut style.min_size.height,
        &mut style.max_size.width,
        &mut style.max_size.height,
        &mut style.margin.top,
        &mut style.margin.right,
        &mut style.margin.bottom,
        &mut style.margin.left,
        &mut style.flex_basis,
    ]
    .into_iter()
    .flatten()
    {
        if let Length::Definite(value) = value {
            definite(value, zoom);
        }
    }
    for value in [
        &mut style.padding.top,
        &mut style.padding.right,
        &mut style.padding.bottom,
        &mut style.padding.left,
        &mut style.gap.width,
        &mut style.gap.height,
    ]
    .into_iter()
    .flatten()
    {
        definite(value, zoom);
    }
    for value in [
        &mut style.border_widths.top,
        &mut style.border_widths.right,
        &mut style.border_widths.bottom,
        &mut style.border_widths.left,
        &mut style.corner_radii.top_left,
        &mut style.corner_radii.top_right,
        &mut style.corner_radii.bottom_left,
        &mut style.corner_radii.bottom_right,
        &mut style.scrollbar_width,
    ]
    .into_iter()
    .flatten()
    {
        absolute(value, zoom);
    }
    for offset in [
        &mut style.paint_translation,
        &mut style.paint_scale_origin_offset,
    ]
    .into_iter()
    .flatten()
    {
        *offset *= zoom;
    }
    if let Some(blur) = &mut style.backdrop_blur {
        *blur *= zoom;
    }
    for operations in [&mut style.backdrop_filters, &mut style.foreground_filters]
        .into_iter()
        .flatten()
    {
        for operation in operations {
            match operation {
                BackdropFilterOperation::Blur(sigma) => *sigma *= zoom,
                BackdropFilterOperation::DropShadow { offset, sigma, .. } => {
                    offset[0] *= zoom;
                    offset[1] *= zoom;
                    *sigma *= zoom;
                }
                _ => {}
            }
        }
    }
    for shadow in style.box_shadow.iter_mut().flatten() {
        shadow.offset.x *= zoom;
        shadow.offset.y *= zoom;
        shadow.blur_radius *= zoom;
        shadow.spread_radius *= zoom;
    }
    if let Some(text) = &mut style.text {
        if let Some(size) = &mut text.font_size {
            absolute(size, zoom);
        }
        if let Some(height) = &mut text.line_height {
            definite(height, zoom);
        }
        if let Some(spacing) = &mut text.letter_spacing {
            *spacing *= zoom;
        }
        for shadow in text.text_shadows.iter_mut().flatten() {
            shadow.offset.x *= zoom;
            shadow.offset.y *= zoom;
            shadow.blur_radius *= zoom;
        }
        if let Some(CssDecorationThickness::Length(thickness)) = &mut text.css_decoration_thickness
        {
            *thickness *= zoom;
        }
        if let Some(underline) = &mut text.underline {
            underline.thickness *= zoom;
        }
        if let Some(overline) = &mut text.overline {
            overline.thickness *= zoom;
        }
        if let Some(strike) = &mut text.strikethrough {
            strike.thickness *= zoom;
        }
    }
}
fn native_filter_operation(
    operation: feishin_core::styles::FilterOperation,
) -> BackdropFilterOperation {
    use feishin_core::styles::FilterOperation as Core;
    match operation {
        Core::Blur(n) => BackdropFilterOperation::Blur(n),
        Core::Brightness(n) => BackdropFilterOperation::Brightness(n),
        Core::Contrast(n) => BackdropFilterOperation::Contrast(n),
        Core::Saturate(n) => BackdropFilterOperation::Saturate(n),
        Core::Grayscale(n) => BackdropFilterOperation::Grayscale(n),
        Core::Sepia(n) => BackdropFilterOperation::Sepia(n),
        Core::HueRotate(n) => BackdropFilterOperation::HueRotate(n),
        Core::Invert(n) => BackdropFilterOperation::Invert(n),
        Core::Opacity(n) => BackdropFilterOperation::Opacity(n),
        Core::DropShadow {
            offset,
            sigma,
            color,
        } => BackdropFilterOperation::DropShadow {
            offset,
            sigma,
            color,
        },
    }
}

fn valid_inherited_css_value(name: &str, value: &str, font: f32) -> bool {
    if matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "inherit" | "initial" | "unset" | "revert"
    ) {
        return true;
    }
    match name {
        "word-break" => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "normal" | "keep-all" | "break-all" | "break-word"
        ),
        "overflow-wrap" => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "normal" | "break-word" | "anywhere"
        ),
        "direction" => matches!(value.trim().to_ascii_lowercase().as_str(), "ltr" | "rtl"),
        "text-spacing-trim" => TextSpacingTrim::parse(value).is_some(),
        "letter-spacing" => {
            value.trim().eq_ignore_ascii_case("normal") || tracking(value, font).is_some()
        }
        "text-shadow" => feishin_core::styles::text_shadows(value, font).is_some(),
        "font-style" => feishin_core::fonts::style_keyword(value).is_some(),
        "font-variant-numeric" => feishin_core::fonts::numeric_features(value).is_some(),
        "font-feature-settings" => feishin_core::fonts::feature_settings(value).is_some(),
        "font-variation-settings" => feishin_core::fonts::variation_settings(value).is_some(),
        "backdrop-filter" | "filter" => {
            feishin_core::styles::backdrop_filter(value, font).is_some()
        }
        _ => false,
    }
}

fn initial_layout_css_value(name: &str) -> Option<&'static str> {
    if logical_spacing_sides(name, false).is_some() {
        return Some("0");
    }
    Some(match name {
        "opacity" | "flex-shrink" => "1",
        "flex-grow" => "0",
        "background-color" => "transparent",
        "background-image" | "box-shadow" | "transform" => "none",
        "transform-origin" => "50% 50%",
        "object-fit" => "fill",
        "border-radius" | "padding" | "padding-top" | "padding-right" | "padding-bottom"
        | "padding-left" | "margin" | "margin-top" | "margin-right" | "margin-bottom"
        | "margin-left" => "0",
        "gap" | "row-gap" | "column-gap" | "align-items" | "align-content" | "justify-content" => {
            "normal"
        }
        "overflow" | "overflow-x" | "overflow-y" => "visible",
        "flex-basis" => "auto",
        "flex" => "0 1 auto",
        "flex-direction" => "row",
        "flex-wrap" => "nowrap",
        "position" => "static",
        _ => return None,
    })
}

fn valid_layout_css_value(
    name: &str,
    value: &str,
    palette: &appearance::Palette,
    font: f32,
) -> bool {
    if matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "inherit" | "initial" | "unset" | "revert" | "revert-layer"
    ) {
        return true;
    }
    let mut parsed = StyleRefinement::default();
    apply_property_value(&mut parsed, name, value, palette, font);
    parsed != StyleRefinement::default()
}

fn tracking(value: &str, font: f32) -> Option<DefiniteLength> {
    match layout_length(value, font) {
        Some(Length::Definite(value)) => Some(value),
        _ => None,
    }
}

fn length(value: &str, font: f32) -> Option<f32> {
    feishin_core::styles::length(value, font, font)
}

fn transform_length(value: &str, font: f32) -> Option<(f32, f32)> {
    feishin_core::styles::length_percentage(value, font, font)
}

fn css_transform(value: &str, font: f32) -> Option<(f32, Point<Pixels>, Point<f32>)> {
    let mut scale = 1.;
    let mut pixels = point(px(0.), px(0.));
    let mut fraction = point(0., 0.);
    if value.trim().eq_ignore_ascii_case("none") {
        return Some((scale, pixels, fraction));
    }
    let functions = feishin_core::styles::component_values(value)?;
    if functions.is_empty() {
        return None;
    }
    for function in functions {
        let (name, args) = function.split_once('(')?;
        let args = args
            .strip_suffix(')')?
            .split(',')
            .map(str::trim)
            .collect::<Vec<_>>();
        let number = |index: usize| {
            args.get(index)?
                .parse::<f32>()
                .ok()
                .filter(|number| number.is_finite())
        };
        let scale_number = |index: usize| {
            if let Some(percent) = args.get(index)?.strip_suffix('%') {
                percent
                    .parse::<f32>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map(|n| n / 100.)
            } else {
                number(index)
            }
        };
        let dimension = |index: usize| transform_length(args.get(index)?, font);
        let mut translation = [(0., 0.); 2];
        let multiplier = match name.to_ascii_lowercase().as_str() {
            "translate" if args.len() == 1 || args.len() == 2 => {
                translation[0] = dimension(0)?;
                if args.len() == 2 {
                    translation[1] = dimension(1)?;
                }
                1.
            }
            "translatex" | "translatey" if args.len() == 1 => {
                translation[usize::from(name.eq_ignore_ascii_case("translatey"))] = dimension(0)?;
                1.
            }
            "translate3d" if args.len() == 3 && length(args[2], font)? == 0. => {
                translation = [dimension(0)?, dimension(1)?];
                1.
            }
            "scale"
                if args.len() == 1 || args.len() == 2 && scale_number(0)? == scale_number(1)? =>
            {
                scale_number(0)?
            }
            "scale3d"
                if args.len() == 3
                    && scale_number(0)? == scale_number(1)?
                    && scale_number(2)? == 1. =>
            {
                scale_number(0)?
            }
            "matrix"
                if args.len() == 6
                    && number(1)? == 0.
                    && number(2)? == 0.
                    && number(0)? == number(3)? =>
            {
                translation = [(number(4)?, 0.), (number(5)?, 0.)];
                number(0)?
            }
            _ => return None,
        };
        pixels += point(px(translation[0].0 * scale), px(translation[1].0 * scale));
        fraction += point(translation[0].1 * scale, translation[1].1 * scale);
        scale *= multiplier;
    }
    (scale.is_finite()
        && (0. ..=64.).contains(&scale)
        && f32::from(pixels.x).is_finite()
        && f32::from(pixels.y).is_finite()
        && fraction.x.is_finite()
        && fraction.y.is_finite())
    .then_some((scale, pixels, fraction))
}

fn css_transform_origin(value: &str, font: f32) -> Option<(Point<Pixels>, Point<f32>)> {
    let values = feishin_core::styles::component_values(&value.to_ascii_lowercase())?;
    let mut parts = values.iter().map(String::as_str).collect::<Vec<_>>();
    if parts.len() == 3 {
        length(parts.pop()?, font)?;
    }
    if parts.len() == 1 {
        parts.push("center");
    }
    if parts.len() != 2 {
        return None;
    }
    if matches!(parts[0], "top" | "bottom") || matches!(parts[1], "left" | "right") {
        // Reversed order is valid only when both coordinates are keywords.
        if !parts
            .iter()
            .all(|part| matches!(*part, "left" | "right" | "top" | "bottom" | "center"))
        {
            return None;
        }
        parts.swap(0, 1);
    }
    let coordinate = |value: &str, vertical: bool| match value {
        "center" => Some((0., 0.5)),
        "left" if !vertical => Some((0., 0.)),
        "right" if !vertical => Some((0., 1.)),
        "top" if vertical => Some((0., 0.)),
        "bottom" if vertical => Some((0., 1.)),
        _ => transform_length(value, font),
    };
    let x = coordinate(parts[0], false)?;
    let y = coordinate(parts[1], true)?;
    Some((point(px(x.0), px(y.0)), point(x.1, y.1)))
}

fn layout_length(value: &str, font: f32) -> Option<Length> {
    let value = value.trim();
    if value == "auto" {
        return Some(Length::Auto);
    }
    let (pixels, fraction) = feishin_core::styles::length_percentage(value, font, font)?;
    Some(if !value.contains('%') {
        px(pixels).into()
    } else if pixels == 0. {
        relative(fraction).into()
    } else {
        DefiniteLength::Calc(px(pixels), fraction).into()
    })
}
fn edges(value: &str, font: f32) -> Option<[f32; 4]> {
    parse_edges(value, |value| length(value, font))
}
fn parse_edges<T: Copy>(value: &str, mut parse: impl FnMut(&str) -> Option<T>) -> Option<[T; 4]> {
    let values = feishin_core::styles::component_values(value)?
        .iter()
        .map(|s| parse(s))
        .collect::<Option<Vec<_>>>()?;
    Some(match values.as_slice() {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    })
}
fn padding_length(value: &str, font: f32) -> Option<DefiniteLength> {
    match nonnegative_layout_length(value, font)? {
        Length::Definite(value) => Some(value),
        Length::Auto => None,
    }
}
fn logical_spacing_sides(name: &str, rtl: bool) -> Option<(&'static str, &'static str)> {
    let (margin, axis) = if let Some(axis) = name.strip_prefix("margin-") {
        (true, axis)
    } else {
        (false, name.strip_prefix("padding-")?)
    };
    let (axis, edge) = axis.split_once('-').map_or((axis, ""), |parts| parts);
    let (start, end) = match (margin, axis, rtl) {
        (true, "inline", false) => ("margin-left", "margin-right"),
        (true, "inline", true) => ("margin-right", "margin-left"),
        (true, "block", _) => ("margin-top", "margin-bottom"),
        (false, "inline", false) => ("padding-left", "padding-right"),
        (false, "inline", true) => ("padding-right", "padding-left"),
        (false, "block", _) => ("padding-top", "padding-bottom"),
        _ => return None,
    };
    match edge {
        "" => Some((start, end)),
        "start" => Some((start, start)),
        "end" => Some((end, end)),
        _ => None,
    }
}

fn spacing_indices(name: &str, rtl: bool) -> Option<(bool, Vec<usize>)> {
    if matches!(name, "margin" | "padding") {
        return Some((name == "margin", vec![0, 1, 2, 3]));
    }
    let (start, end) = logical_spacing_sides(name, rtl).unwrap_or((name, name));
    let edge = |name: &str| {
        let (kind, side) = name.split_once('-')?;
        if !matches!(kind, "margin" | "padding") {
            return None;
        }
        Some(match side {
            "top" => 0,
            "right" => 1,
            "bottom" => 2,
            "left" => 3,
            _ => return None,
        })
    };
    let mut indices = vec![edge(start)?];
    if end != start {
        indices.push(edge(end)?);
    }
    Some((name.starts_with("margin-"), indices))
}

fn inherit_spacing(style: &mut StyleRefinement, parent: &StyleRefinement, name: &str) {
    let rtl = |style: &StyleRefinement| {
        style.text.as_ref().and_then(|text| text.text_direction) == Some(TextDirection::Rtl)
    };
    let (margin, destinations) = spacing_indices(name, rtl(style)).unwrap();
    // Logical declarations map to the child's physical edges before inheritance.
    if margin {
        let values = [
            parent.margin.top,
            parent.margin.right,
            parent.margin.bottom,
            parent.margin.left,
        ];
        let fields = [
            &mut style.margin.top,
            &mut style.margin.right,
            &mut style.margin.bottom,
            &mut style.margin.left,
        ];
        for edge in destinations {
            *fields[edge] = Some(values[edge].unwrap_or(px(0.).into()));
        }
    } else {
        let values = [
            parent.padding.top,
            parent.padding.right,
            parent.padding.bottom,
            parent.padding.left,
        ];
        let fields = [
            &mut style.padding.top,
            &mut style.padding.right,
            &mut style.padding.bottom,
            &mut style.padding.left,
        ];
        for edge in destinations {
            *fields[edge] = Some(values[edge].unwrap_or(px(0.).into()));
        }
    }
}
fn nonnegative_layout_length(value: &str, font: f32) -> Option<Length> {
    let length = layout_length(value, font)?;
    match length {
        Length::Auto | Length::Definite(DefiniteLength::Calc(..)) => Some(length),
        Length::Definite(length) if length.to_pixels(px(1.).into(), px(1.)) >= px(0.) => {
            Some(length.into())
        }
        _ if value
            .trim()
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("calc(")) =>
        {
            Some(
                if matches!(length, Length::Definite(DefiniteLength::Fraction(_))) {
                    relative(0.).into()
                } else {
                    px(0.).into()
                },
            )
        }
        _ => None,
    }
}
fn border_width(value: &str, font: f32) -> Option<f32> {
    match value {
        "thin" => Some(1.),
        "medium" => Some(3.),
        "thick" => Some(5.),
        _ => length(value, font).filter(|width| *width >= 0.),
    }
}
fn valid_border(value: &str, palette: &appearance::Palette, font: f32) -> bool {
    let Some(parts) = feishin_core::styles::component_values(value) else {
        return false;
    };
    let mut seen = [false; 3];
    !parts.is_empty()
        && parts.iter().all(|part| {
            let kind = if border_width(part, font).is_some() {
                0
            } else if matches!(
                part.as_str(),
                "none"
                    | "hidden"
                    | "solid"
                    | "dashed"
                    | "dotted"
                    | "double"
                    | "groove"
                    | "ridge"
                    | "inset"
                    | "outset"
            ) {
                1
            } else if color(part, palette).is_some() || part.eq_ignore_ascii_case("currentcolor") {
                2
            } else {
                return false;
            };
            !std::mem::replace(&mut seen[kind], true)
        })
}
fn color(value: &str, palette: &appearance::Palette) -> Option<Hsla> {
    let [r, g, b, a] = appearance::theme_color(value, palette).ok()?;
    Some(Rgba { r, g, b, a }.into())
}
fn decoration_shorthand(
    value: &str,
    palette: &appearance::Palette,
    font: f32,
) -> Option<BTreeMap<String, String>> {
    let mut properties: BTreeMap<String, String> = [
        ("text-decoration-line", "none"),
        ("text-decoration-color", "currentColor"),
        ("text-decoration-style", "solid"),
        ("text-decoration-thickness", "auto"),
    ]
    .into_iter()
    .map(|(key, value)| (key.into(), value.into()))
    .collect();
    let mut lines = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for part in feishin_core::styles::component_values(value)? {
        let property = match part.as_str() {
            "none" | "underline" | "overline" | "line-through" => {
                if lines.contains(&part) {
                    return None;
                }
                lines.push(part);
                continue;
            }
            "solid" | "wavy" | "dotted" | "dashed" | "double" => "text-decoration-style",
            "auto" | "from-font" => "text-decoration-thickness",
            "currentColor" | "currentcolor" => "text-decoration-color",
            _ if color(&part, palette).is_some() => "text-decoration-color",
            _ if length(&part, font).is_some_and(|thickness| thickness >= 0.) => {
                "text-decoration-thickness"
            }
            _ => return None,
        };
        if !seen.insert(property) {
            return None;
        }
        properties.insert(property.into(), part);
    }
    if lines.len() > 1 && lines.iter().any(|line| line == "none") {
        return None;
    }
    if !lines.is_empty() {
        properties.insert("text-decoration-line".into(), lines.join(" "));
    }
    Some(properties)
}
fn valid_decoration_longhand(
    name: &str,
    value: &str,
    palette: &appearance::Palette,
    font: f32,
) -> bool {
    let Some(properties) = decoration_shorthand(value, palette, font) else {
        return false;
    };
    match name {
        "text-decoration-line" => feishin_core::styles::component_values(value)
            .is_some_and(|parts| parts.join(" ") == properties["text-decoration-line"]),
        "text-decoration-color" => {
            color(value, palette).is_some() || value.eq_ignore_ascii_case("currentcolor")
        }
        "text-decoration-thickness" => {
            length(value, font).is_some() || matches!(value, "auto" | "from-font")
        }
        "text-decoration-style" => {
            matches!(value, "solid" | "dashed" | "dotted" | "double" | "wavy")
        }
        _ => false,
    }
}
fn shadows(value: &str, palette: &appearance::Palette, font: f32) -> Option<Vec<BoxShadow>> {
    let parts = feishin_core::styles::component_values(value)?;
    parts
        .split(|part| part == ",")
        .map(|parts| {
            let mut lengths = Vec::new();
            let mut shade = None;
            for part in parts {
                if let Some(length) = length(part, font) {
                    lengths.push(length);
                } else if shade.is_none() {
                    shade = Some(color(part, palette)?);
                } else {
                    return None;
                }
            }
            if !(2..=4).contains(&lengths.len()) || lengths.get(2).is_some_and(|blur| *blur < 0.) {
                return None;
            }
            Some(BoxShadow {
                color: shade.unwrap_or_else(|| {
                    Rgba {
                        r: 0.,
                        g: 0.,
                        b: 0.,
                        a: 1.,
                    }
                    .into()
                }),
                offset: point(px(lengths[0]), px(lengths[1])),
                blur_radius: px(lengths.get(2).copied().unwrap_or(0.)),
                spread_radius: px(lengths.get(3).copied().unwrap_or(0.)),
            })
        })
        .collect()
}
fn apply_property(
    style: &mut StyleRefinement,
    name: &str,
    value: &str,
    palette: &appearance::Palette,
    font: f32,
) {
    let value = if matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "initial" | "unset" | "revert" | "revert-layer"
    ) {
        initial_layout_css_value(name).unwrap_or(value)
    } else {
        value
    };
    // Invalid values produce no refinement and retain the earlier cascade.
    let mut next = style.clone();
    apply_property_value(&mut next, name, value, palette, font);
    if let Some(text) = next.text.take() {
        style
            .text
            .get_or_insert_with(Default::default)
            .refine(&text);
    }
    style.refine(&next);
}
fn apply_property_value(
    style: &mut StyleRefinement,
    name: &str,
    value: &str,
    palette: &appearance::Palette,
    font: f32,
) {
    if let Some((start, end)) = logical_spacing_sides(
        name,
        style.text.as_ref().and_then(|text| text.text_direction) == Some(TextDirection::Rtl),
    ) {
        let Some(parts) = feishin_core::styles::component_values(value) else {
            return;
        };
        if parts.is_empty()
            || parts.len() > if start == end { 1 } else { 2 }
            || !parts.iter().all(|value| {
                if start.starts_with("margin-") {
                    layout_length(value, font).is_some()
                } else {
                    padding_length(value, font).is_some()
                }
            })
        {
            return;
        }
        apply_property_value(style, start, &parts[0], palette, font);
        apply_property_value(style, end, parts.last().unwrap(), palette, font);
        return;
    }
    let value = if value == "normal" && matches!(name, "gap" | "row-gap" | "column-gap") {
        "0"
    } else {
        value
    };
    if matches!(
        name,
        "border" | "border-top" | "border-right" | "border-bottom" | "border-left"
    ) && !valid_border(value, palette, font)
    {
        return;
    }
    if matches!(
        name,
        "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width"
    ) && border_width(value, font).is_none()
    {
        return;
    }
    match name {
        "transform" => {
            if let Some((scale, pixels, fraction)) = css_transform(value, font) {
                style.paint_scale = Some(scale);
                style.paint_translation = Some(pixels);
                style.paint_translation_fraction = Some(fraction);
            }
        }
        "transform-origin" => {
            if let Some((pixels, fraction)) = css_transform_origin(value, font) {
                style.paint_scale_origin = Some(fraction);
                style.paint_scale_origin_offset = Some(pixels);
            }
        }
        "background" | "background-color" | "background-image" => {
            if let Some(color) = color(value, palette) {
                if name == "background-image" {
                    return;
                }
                if name == "background-color"
                    && let Some(Fill::Image(image)) = &mut style.background
                {
                    image.color = Some(color.into());
                } else {
                    style.background = Some(color.into());
                }
            } else if value == "none" && name != "background-color" {
                let under = if name == "background-image" {
                    style.background.as_ref().and_then(Fill::color)
                } else {
                    None
                };
                style.background = Some(Fill::Color(
                    under.unwrap_or_else(|| transparent_black().into()),
                ));
            } else if name != "background-color"
                && let Ok(gradient) = appearance::gradient(value, palette)
            {
                let mut image = crate::appearance::gradient_fill(gradient);
                if name == "background-image" {
                    image.color = style.background.as_ref().and_then(Fill::color);
                    image.inherit_background_color = true;
                }
                style.background = Some(Fill::Image(Box::new(image)));
            }
        }
        "color" => style.text.get_or_insert_with(Default::default).color = color(value, palette),
        "border-color" => style.border_color = color(value, palette),
        "opacity" => {
            if let Ok(n) = value.parse::<f32>()
                && n.is_finite()
            {
                style.opacity = Some(n.clamp(0., 1.));
            }
        }
        "border-radius" => {
            if value.trim().ends_with('%')
                && value
                    .trim_end_matches('%')
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .is_some_and(|value| value >= 50.)
            {
                style.corner_radii = CornersRefinement {
                    top_left: Some(px(10_000.).into()),
                    top_right: Some(px(10_000.).into()),
                    bottom_left: Some(px(10_000.).into()),
                    bottom_right: Some(px(10_000.).into()),
                };
            } else if let Some([a, b, c, d]) =
                edges(value, font).filter(|values| values.iter().all(|n| *n >= 0.))
            {
                style.corner_radii = CornersRefinement {
                    top_left: Some(px(a.max(0.)).into()),
                    top_right: Some(px(b.max(0.)).into()),
                    bottom_right: Some(px(c.max(0.)).into()),
                    bottom_left: Some(px(d.max(0.)).into()),
                };
            }
        }
        "border" | "border-width" => {
            if name == "border" {
                if matches!(value, "none" | "hidden") {
                    apply_property(style, "border-width", "0", palette, font);
                    return;
                }
                if let Some(parts) = feishin_core::styles::component_values(value) {
                    for part in parts {
                        if part == "dashed" {
                            style.border_style = Some(BorderStyle::Dashed);
                        } else if part == "solid" {
                            style.border_style = Some(BorderStyle::Solid);
                        } else if let Some(color) = color(&part, palette) {
                            style.border_color = Some(color);
                        }
                    }
                }
            }
            let widths = if name == "border-width" {
                edges(value, font)
            } else {
                feishin_core::styles::component_values(value)
                    .and_then(|parts| parts.iter().find_map(|part| border_width(part, font)))
                    .map(|n| [n; 4])
            };
            if let Some([top, right, bottom, left]) =
                widths.filter(|values| values.iter().all(|n| *n >= 0.))
            {
                style.border_widths = EdgesRefinement {
                    top: Some(px(top.max(0.)).into()),
                    right: Some(px(right.max(0.)).into()),
                    bottom: Some(px(bottom.max(0.)).into()),
                    left: Some(px(left.max(0.)).into()),
                };
            }
        }
        "width" => style.size.width = nonnegative_layout_length(value, font),
        "border-top"
        | "border-right"
        | "border-bottom"
        | "border-left"
        | "border-top-width"
        | "border-right-width"
        | "border-bottom-width"
        | "border-left-width" => {
            let width = if matches!(value, "none" | "hidden") {
                Some(0.)
            } else {
                feishin_core::styles::component_values(value)
                    .and_then(|parts| parts.iter().find_map(|part| border_width(part, font)))
            };
            if let Some(width) = width {
                let width = Some(px(width.max(0.)).into());
                match name {
                    "border-top" | "border-top-width" => style.border_widths.top = width,
                    "border-right" | "border-right-width" => style.border_widths.right = width,
                    "border-bottom" | "border-bottom-width" => style.border_widths.bottom = width,
                    _ => style.border_widths.left = width,
                }
            }
            if !name.ends_with("-width")
                && let Some(parts) = feishin_core::styles::component_values(value)
            {
                for part in parts {
                    if let Some(color) = color(&part, palette) {
                        style.border_color = Some(color);
                    }
                }
            }
        }
        "inset" => {
            if let Some([top, right, bottom, left]) = edges(value, font) {
                style.inset = EdgesRefinement {
                    top: Some(px(top).into()),
                    right: Some(px(right).into()),
                    bottom: Some(px(bottom).into()),
                    left: Some(px(left).into()),
                };
            }
        }
        "position" => {
            style.position = match value {
                "absolute" => Some(Position::Absolute),
                "relative" | "static" => Some(Position::Relative),
                _ => None,
            }
        }
        "top" => style.inset.top = layout_length(value, font),
        "right" => style.inset.right = layout_length(value, font),
        "bottom" => style.inset.bottom = layout_length(value, font),
        "left" => style.inset.left = layout_length(value, font),
        "aspect-ratio" => {
            let (width, height) = value
                .split_once('/')
                .map_or((value, "1"), |(w, h)| (w.trim(), h.trim()));
            style.aspect_ratio = width
                .parse::<f32>()
                .ok()
                .zip(height.parse::<f32>().ok())
                .map(|(w, h)| w / h)
                .filter(|n| n.is_finite() && *n > 0. && *n < 10000.);
        }
        "object-fit" => {
            style.object_fit = match value.to_ascii_lowercase().as_str() {
                "fill" => Some(gpui::ObjectFit::Fill),
                "contain" => Some(gpui::ObjectFit::Contain),
                "cover" => Some(gpui::ObjectFit::Cover),
                "none" => Some(gpui::ObjectFit::None),
                "scale-down" => Some(gpui::ObjectFit::ScaleDown),
                _ => None,
            };
        }
        "border-style" => {
            style.border_style = match value {
                "solid" => Some(BorderStyle::Solid),
                "dashed" => Some(BorderStyle::Dashed),
                _ => None,
            }
        }
        "height" => style.size.height = nonnegative_layout_length(value, font),
        "min-width" => style.min_size.width = nonnegative_layout_length(value, font),
        "min-height" => style.min_size.height = nonnegative_layout_length(value, font),
        "max-width" => style.max_size.width = nonnegative_layout_length(value, font),
        "max-height" => style.max_size.height = nonnegative_layout_length(value, font),
        "padding" => {
            if let Some([a, b, c, d]) = parse_edges(value, |value| padding_length(value, font)) {
                style.padding = EdgesRefinement {
                    top: Some(a),
                    right: Some(b),
                    bottom: Some(c),
                    left: Some(d),
                };
            }
        }
        "margin" => {
            if let Some([a, b, c, d]) = parse_edges(value, |value| layout_length(value, font)) {
                style.margin = EdgesRefinement {
                    top: Some(a),
                    right: Some(b),
                    bottom: Some(c),
                    left: Some(d),
                };
            }
        }
        "padding-top" => style.padding.top = padding_length(value, font),
        "padding-right" => style.padding.right = padding_length(value, font),
        "padding-bottom" => style.padding.bottom = padding_length(value, font),
        "padding-left" => style.padding.left = padding_length(value, font),
        "margin-top" => style.margin.top = layout_length(value, font),
        "margin-right" => style.margin.right = layout_length(value, font),
        "margin-bottom" => style.margin.bottom = layout_length(value, font),
        "margin-left" => style.margin.left = layout_length(value, font),
        "gap" => {
            if let Some(n) = length(value, font).filter(|n| *n >= 0.) {
                style.gap.width = Some(px(n.max(0.)).into());
                style.gap.height = Some(px(n.max(0.)).into());
            }
        }
        "font-size" => {
            style.text.get_or_insert_with(Default::default).font_size = length(value, font)
                .filter(|n| *n >= 0.)
                .map(|n| px(n).into())
        }
        "font-family" => {
            if let Some(mut families) = feishin_core::fonts::family_list(value) {
                let text = style.text.get_or_insert_with(Default::default);
                text.font_family = Some(families.remove(0).into());
                text.font_fallbacks = Some(FontFallbacks::from_fonts(families));
            }
        }
        "font-weight" => {
            let n = match value {
                "normal" => Some(400.),
                "bold" => Some(700.),
                _ => value.parse::<f32>().ok(),
            };
            if let Some(n) = n.filter(|n| n.is_finite() && (1. ..=1000.).contains(n)) {
                style.text.get_or_insert_with(Default::default).font_weight = Some(FontWeight(n));
            }
        }
        "font-style" => {
            let parsed = match feishin_core::fonts::style_keyword(value) {
                Some("italic") => Some(FontStyle::Italic),
                Some("normal" | "initial") => Some(FontStyle::Normal),
                Some("oblique") => Some(FontStyle::Oblique),
                _ => None,
            };
            if let Some(parsed) = parsed {
                style.text.get_or_insert_with(Default::default).font_style = Some(parsed);
            }
        }
        "text-decoration-line" => {
            if let Some(parts) = feishin_core::styles::component_values(value) {
                let underline = parts.iter().any(|part| part == "underline");
                let strike = parts.iter().any(|part| part == "line-through");
                let overline = parts.iter().any(|part| part == "overline");
                let none = parts.iter().any(|part| part == "none");
                if !parts.is_empty()
                    && parts.iter().all(|part| {
                        matches!(
                            part.as_str(),
                            "none" | "underline" | "overline" | "line-through"
                        )
                    })
                    && (!none || parts.len() == 1)
                {
                    let text = style.text.get_or_insert_with(Default::default);
                    text.css_decoration_thickness = Some(CssDecorationThickness::Auto);
                    // Zero-width decorations also clear inherited native decoration styles.
                    text.underline = Some(UnderlineStyle {
                        thickness: px(if underline { 1. } else { 0. }),
                        ..Default::default()
                    });
                    text.overline = Some(Box::new(UnderlineStyle {
                        thickness: px(if overline { 1. } else { 0. }),
                        ..Default::default()
                    }));
                    text.strikethrough = Some(StrikethroughStyle {
                        thickness: px(if strike { 1. } else { 0. }),
                        ..Default::default()
                    });
                }
            }
        }
        "text-decoration-color" | "text-decoration-thickness" | "text-decoration-style" => {
            let text = style.text.get_or_insert_with(Default::default);
            if name == "text-decoration-color" {
                let color = color(value, palette);
                if let Some(underline) = text.underline.as_mut() {
                    underline.color = color;
                }
                if let Some(overline) = text.overline.as_mut() {
                    overline.color = color;
                }
                if let Some(strike) = text.strikethrough.as_mut() {
                    strike.color = color;
                }
            } else if name == "text-decoration-thickness" {
                let thickness = match value {
                    "auto" => Some(CssDecorationThickness::Auto),
                    "from-font" => Some(CssDecorationThickness::FromFont),
                    _ => length(value, font)
                        .filter(|value| *value >= 0.)
                        .map(|value| CssDecorationThickness::Length(px(value))),
                };
                if let Some(thickness) = thickness {
                    text.css_decoration_thickness = Some(thickness);
                }
                if let Some(thickness) = length(value, font).filter(|value| *value >= 0.) {
                    if let Some(underline) = text
                        .underline
                        .as_mut()
                        .filter(|style| style.thickness > px(0.))
                    {
                        underline.thickness = px(thickness);
                    }
                    if let Some(overline) = text
                        .overline
                        .as_mut()
                        .filter(|style| style.thickness > px(0.))
                    {
                        overline.thickness = px(thickness);
                    }
                    if let Some(strike) = text
                        .strikethrough
                        .as_mut()
                        .filter(|style| style.thickness > px(0.))
                    {
                        strike.thickness = px(thickness);
                    }
                }
            } else {
                let pattern = match value {
                    "solid" => Some(TextDecorationPattern::Solid),
                    "wavy" => Some(TextDecorationPattern::Wavy),
                    "dotted" => Some(TextDecorationPattern::Dotted),
                    "dashed" => Some(TextDecorationPattern::Dashed),
                    "double" => Some(TextDecorationPattern::Double),
                    _ => None,
                };
                if let Some(pattern) = pattern {
                    if let Some(underline) = text.underline.as_mut() {
                        underline.pattern = pattern;
                        underline.wavy = pattern == TextDecorationPattern::Wavy;
                    }
                    if let Some(overline) = text.overline.as_mut() {
                        overline.pattern = pattern;
                        overline.wavy = pattern == TextDecorationPattern::Wavy;
                    }
                    if let Some(strike) = text.strikethrough.as_mut() {
                        strike.pattern = pattern;
                    }
                }
            }
        }
        "cursor" => {
            style.mouse_cursor = match value {
                "default" => Some(CursorStyle::Arrow),
                "text" => Some(CursorStyle::IBeam),
                "vertical-text" => Some(CursorStyle::IBeamCursorForVerticalLayout),
                "pointer" => Some(CursorStyle::PointingHand),
                "crosshair" => Some(CursorStyle::Crosshair),
                "grab" => Some(CursorStyle::OpenHand),
                "grabbing" => Some(CursorStyle::ClosedHand),
                "not-allowed" => Some(CursorStyle::OperationNotAllowed),
                "alias" => Some(CursorStyle::DragLink),
                "copy" => Some(CursorStyle::DragCopy),
                "context-menu" => Some(CursorStyle::ContextualMenu),
                "w-resize" => Some(CursorStyle::ResizeLeft),
                "e-resize" => Some(CursorStyle::ResizeRight),
                "ew-resize" => Some(CursorStyle::ResizeLeftRight),
                "n-resize" => Some(CursorStyle::ResizeUp),
                "s-resize" => Some(CursorStyle::ResizeDown),
                "ns-resize" => Some(CursorStyle::ResizeUpDown),
                "nwse-resize" | "nw-resize" | "se-resize" => {
                    Some(CursorStyle::ResizeUpLeftDownRight)
                }
                "nesw-resize" | "ne-resize" | "sw-resize" => {
                    Some(CursorStyle::ResizeUpRightDownLeft)
                }
                "col-resize" => Some(CursorStyle::ResizeColumn),
                "row-resize" => Some(CursorStyle::ResizeRow),
                "none" => Some(CursorStyle::None),
                _ => None,
            }
        }
        "white-space" => {
            style.text.get_or_insert_with(Default::default).white_space = match value {
                "nowrap" | "pre" => Some(WhiteSpace::Nowrap),
                "normal" | "pre-wrap" | "pre-line" => Some(WhiteSpace::Normal),
                _ => None,
            }
        }
        "text-overflow" => {
            style
                .text
                .get_or_insert_with(Default::default)
                .text_overflow = match value {
                "ellipsis" => Some(TextOverflow::Truncate("…".into())),
                "clip" => Some(TextOverflow::Truncate("".into())),
                _ => None,
            }
        }
        "line-height" => {
            let height = value
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite() && *n >= 0. && *n <= 64.)
                .map(relative)
                .or_else(|| {
                    length(value, font)
                        .filter(|n| *n >= 0.)
                        .map(|n| px(n).into())
                });
            style.text.get_or_insert_with(Default::default).line_height = height;
        }
        "text-align" => {
            style.text.get_or_insert_with(Default::default).text_align = match value {
                "left" => Some(TextAlign::Left),
                "right" => Some(TextAlign::Right),
                "start" => Some(TextAlign::Start),
                "end" => Some(TextAlign::End),
                "center" => Some(TextAlign::Center),
                _ => None,
            }
        }
        "display" => {
            style.display = match value {
                "none" => Some(Display::None),
                "flex" => Some(Display::Flex),
                "block" => Some(Display::Block),
                "grid" => Some(Display::Grid),
                _ => None,
            }
        }
        "visibility" => {
            style.visibility = match value {
                "hidden" => Some(Visibility::Hidden),
                "visible" => Some(Visibility::Visible),
                _ => None,
            }
        }
        "overflow" | "overflow-x" | "overflow-y" => {
            let v = match value {
                "hidden" => Some(Overflow::Hidden),
                "scroll" | "auto" => Some(Overflow::Scroll),
                "visible" => Some(Overflow::Visible),
                _ => None,
            };
            if name != "overflow-y" {
                style.overflow.x = v;
            }
            if name != "overflow-x" {
                style.overflow.y = v;
            }
        }
        "flex-direction" => {
            style.flex_direction = match value {
                "row" => Some(FlexDirection::Row),
                "column" => Some(FlexDirection::Column),
                "row-reverse" => Some(FlexDirection::RowReverse),
                "column-reverse" => Some(FlexDirection::ColumnReverse),
                _ => None,
            }
        }
        "flex-wrap" => {
            style.flex_wrap = match value {
                "nowrap" => Some(FlexWrap::NoWrap),
                "wrap" => Some(FlexWrap::Wrap),
                "wrap-reverse" => Some(FlexWrap::WrapReverse),
                _ => None,
            }
        }
        "flex-basis" => style.flex_basis = nonnegative_layout_length(value, font),
        "flex" => {
            let shorthand = match value {
                "none" => "0 0 auto",
                "auto" => "1 1 auto",
                "initial" | "unset" => "0 1 auto",
                _ => value,
            };
            let parts = feishin_core::styles::component_values(shorthand).unwrap_or_default();
            let number = |value: &str| {
                value
                    .parse::<f32>()
                    .ok()
                    .filter(|n| n.is_finite() && *n >= 0. && *n <= 10000.)
            };
            let basis = |value: &str| nonnegative_layout_length(value, font);
            let parsed = (|| {
                Some(match parts.as_slice() {
                    [grow] if number(grow).is_some() => (number(grow)?, 1., relative(0.).into()),
                    [value] => (1., 1., basis(value)?),
                    [grow, shrink] if number(grow).is_some() && number(shrink).is_some() => {
                        (number(grow)?, number(shrink)?, relative(0.).into())
                    }
                    [grow, value] if number(grow).is_some() => (number(grow)?, 1., basis(value)?),
                    [value, grow] => (number(grow)?, 1., basis(value)?),
                    [grow, shrink, value] if number(grow).is_some() => {
                        (number(grow)?, number(shrink)?, basis(value)?)
                    }
                    [value, grow, shrink] => (number(grow)?, number(shrink)?, basis(value)?),
                    _ => return None,
                })
            })();
            if let Some((grow, shrink, basis)) = parsed {
                style.flex_grow = Some(grow);
                style.flex_shrink = Some(shrink);
                style.flex_basis = Some(basis);
            }
        }
        "row-gap" => {
            style.gap.height = length(value, font)
                .filter(|n| *n >= 0.)
                .map(|n| px(n).into())
        }
        "column-gap" => {
            style.gap.width = length(value, font)
                .filter(|n| *n >= 0.)
                .map(|n| px(n).into())
        }
        "flex-grow" => {
            style.flex_grow = value
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite() && *n >= 0. && *n <= 10000.)
        }
        "flex-shrink" => {
            style.flex_shrink = value
                .parse::<f32>()
                .ok()
                .filter(|n| n.is_finite() && *n >= 0. && *n <= 10000.)
        }
        "align-items" | "align-self" => {
            let alignment = match value {
                "center" => Some(AlignItems::Center),
                "start" => Some(AlignItems::Start),
                "end" => Some(AlignItems::End),
                "flex-start" => Some(AlignItems::FlexStart),
                "flex-end" => Some(AlignItems::FlexEnd),
                "baseline" => Some(AlignItems::Baseline),
                "stretch" | "normal" => Some(AlignItems::Stretch),
                _ => None,
            };
            if name == "align-items" {
                style.align_items = alignment;
            } else {
                style.align_self = alignment;
            }
        }
        "justify-content" | "align-content" => {
            let alignment = match value {
                "center" => Some(JustifyContent::Center),
                "start" => Some(JustifyContent::Start),
                "end" => Some(JustifyContent::End),
                "flex-start" => Some(JustifyContent::FlexStart),
                "flex-end" => Some(JustifyContent::FlexEnd),
                "space-between" => Some(JustifyContent::SpaceBetween),
                "space-around" => Some(JustifyContent::SpaceAround),
                "space-evenly" => Some(JustifyContent::SpaceEvenly),
                "stretch" | "normal" => Some(JustifyContent::Stretch),
                _ => None,
            };
            if name == "justify-content" {
                style.justify_content = alignment;
            } else {
                style.align_content = alignment;
            }
        }
        "box-shadow" if value == "none" => style.box_shadow = Some(vec![]),
        "box-shadow" => style.box_shadow = shadows(value, palette, font),
        _ => {}
    }
}
pub(crate) fn css_border_width(width: Pixels, cx: &App) -> Pixels {
    let scale = cx
        .try_global::<NativeStyles>()
        .map_or(1., |styles| styles.display_scale);
    snapped_border_width(width, scale)
}

fn snapped_border_width(width: Pixels, scale: f32) -> Pixels {
    let width = f32::from(width);
    px(if width > 0. {
        (width * scale).floor().max(1.) / scale
    } else {
        0.
    })
}

pub fn apply<T: Styled>(
    element: T,
    tag: &str,
    id: &str,
    classes: &[&str],
    extra: Option<Target>,
    cx: &App,
) -> T {
    apply_in(element, Target::new(tag, id, classes), extra, cx)
}

pub(crate) fn apply_in<T: Styled>(
    mut element: T,
    target: Target,
    parents: impl IntoIterator<Item = Target>,
    cx: &App,
) -> T {
    if let Some(styles) = cx.try_global::<NativeStyles>() {
        let refinement = styles.style_in(target, parents, f32::from(cx.theme().font_size));
        element = element.refine_style(&refinement);
    }
    element
}
pub(crate) fn icon_stylesheet() -> String {
    include_str!("../assets/styles/icon.css")
        .split("\nimg.size-xs")
        .next()
        .unwrap()
        .replace(".size-", ".fs-icon-module-size-")
}

pub fn play_control(
    id: impl Into<ElementId>,
    mode: feishin_core::queue::Enqueue,
    zoom: f32,
    cx: &App,
) -> Button {
    play_control_in(id, mode, zoom, Vec::new(), cx)
}

pub(crate) fn play_control_in(
    id: impl Into<ElementId>,
    mode: feishin_core::queue::Enqueue,
    zoom: f32,
    mut parents: Vec<Target>,
    cx: &App,
) -> Button {
    parents.push(Target::new(
        "div",
        "",
        &["fs-play-button-group-module-play-button-group"],
    ));
    play_control_with_classes(id, mode, zoom, parents, &[], cx)
}

pub(crate) fn play_control_with_classes(
    id: impl Into<ElementId>,
    mode: feishin_core::queue::Enqueue,
    zoom: f32,
    parents: Vec<Target>,
    classes: &[&str],
    cx: &App,
) -> Button {
    use feishin_core::queue::Enqueue;
    use gpui_component::{
        Icon, Sizable,
        button::{ButtonCustomVariant, ButtonVariants},
    };
    let id = id.into();
    let name = id.to_string();
    let selector = name.clone();
    let icon_selector = format!("{name}-icon");
    let primary = matches!(mode, Enqueue::Now);
    let (icon, label) = match mode {
        Enqueue::Now => ("native-icons/play.svg", "player.play"),
        Enqueue::Next => ("native-icons/play-next.svg", "player.addNext"),
        Enqueue::End => ("native-icons/play-last.svg", "player.addLast"),
    };
    let mut target = Target::new(
        "button",
        &name,
        &[
            "fs-play-button-module-play-button",
            if primary {
                "fs-play-button-module-fill"
            } else {
                "fs-play-button-module-secondary"
            },
        ],
    );
    target
        .classes
        .extend(classes.iter().map(|class| (*class).into()));
    let loading_size = cx
        .try_global::<NativeStyles>()
        .and_then(|styles| {
            styles
                .style_in(
                    Target::new(
                        "svg",
                        "",
                        &["fs-icon-module-size-xl", "fs-spinner-module-icon"],
                    ),
                    parents.iter().cloned().chain([target.clone()]),
                    f32::from(cx.theme().font_size),
                )
                .text
                .and_then(|text| text.font_size)
                .map(|size| size.to_pixels(cx.theme().font_size))
        })
        .unwrap_or(cx.theme().font_size * 1.125);
    Button::new(id)
        .interaction_scale()
        .loading_icon(Icon::default().path("native-icons/spinner.svg"))
        .loading_icon_size(loading_size)
        .debug_selector(move || selector.clone())
        .long_press_feedback(cx.theme().primary)
        .state_style(state_style_path(target, parents))
        .custom(
            ButtonCustomVariant::new(cx)
                .color(white())
                .foreground(black())
                .hover(white())
                .active(white()),
        )
        .with_size(px(if primary { 48. } else { 32. } * zoom))
        .rounded(px(24. * zoom))
        .size(px(if primary { 48. } else { 32. } * zoom))
        .border_0()
        .p_0()
        .child(
            div()
                .id(SharedString::from(icon_selector.clone()))
                .debug_selector(move || icon_selector.clone())
                .size(cx.theme().font_size)
                .child(Icon::default().path(icon).with_size(cx.theme().font_size)),
        )
        .tooltip(format!(
            "{}\n{}",
            crate::appearance::text(label, cx),
            crate::appearance::text("player.holdToShuffle", cx)
        ))
}

pub fn slider(
    state: &Entity<gpui_component::slider::SliderState>,
    cx: &App,
) -> gpui_component::slider::Slider {
    gpui_component::slider::Slider::new(state)
        .rtl(crate::appearance::rtl(cx))
        .when_some(cx.try_global::<NativeStyles>(), |slider, styles| {
            slider.track_background_style(styles.style_in(
                Target::pseudo("before"),
                [
                    Target::new("div", "", &["mantine-Slider-root"]),
                    Target::new("div", "", &["mantine-Slider-track"]),
                ],
                f32::from(cx.theme().font_size),
            ))
        })
}

pub fn button(id: impl Into<ElementId>, cx: &App) -> Button {
    let id = id.into();
    let name = id.to_string();
    let target = Target::new(
        "button",
        &name,
        &["fs-button-module-root", "mantine-Button-root"],
    );
    let loading_size = cx
        .try_global::<NativeStyles>()
        .and_then(|styles| {
            styles
                .style_in(
                    Target::new(
                        "svg",
                        "",
                        &["fs-icon-module-size-xl", "fs-spinner-module-icon"],
                    ),
                    [target.clone()],
                    f32::from(cx.theme().font_size),
                )
                .text
                .and_then(|text| text.font_size)
                .map(|size| size.to_pixels(cx.theme().font_size))
        })
        .unwrap_or(cx.theme().font_size * 1.125);
    Button::new(id)
        .loading_icon(gpui_component::Icon::default().path("native-icons/spinner.svg"))
        .loading_icon_size(loading_size)
        .wrapped_button(
            cx.theme().font_size * (3. / 16.),
            cx.try_global::<NativeStyles>()
                .map_or(1., NativeStyles::zoom),
            cx.theme().font_size / 16.
                * cx.try_global::<NativeStyles>()
                    .and_then(|styles| styles.variables.get("--mantine-scale"))
                    .and_then(|value| value.parse::<f32>().ok())
                    .filter(|value| value.is_finite() && *value > 0.)
                    .unwrap_or(1.),
        )
        .state_style(state_style(target.clone()))
        .paint_motion(std::rc::Rc::new(move |states, cx| {
            cx.try_global::<NativeStyles>()
                .map(|styles| styles.button_motion(target_with_states(target.clone(), states)))
        }))
}
pub fn input(state: &Entity<gpui_component::input::InputState>, cx: &App) -> Input {
    let single_line = state.read(cx).is_single_line();
    let target = Target::new(
        if single_line { "input" } else { "textarea" },
        &state.entity_id().to_string(),
        if single_line {
            &["fs-text-input-module-input", "mantine-Input-input"]
        } else {
            &["fs-textarea-module-input", "mantine-Input-input"]
        },
    );
    apply_in(
        Input::new(state).state_style(state_style(target.clone())),
        target,
        None,
        cx,
    )
}
pub fn select<D: gpui_component::select::SelectDelegate + 'static>(
    state: &Entity<gpui_component::select::SelectState<D>>,
) -> gpui_component::select::Select<D> {
    let id = state.entity_id().to_string();
    gpui_component::select::Select::new(state).state_style(std::rc::Rc::new(move |states, cx| {
        let Some(styles) = cx.try_global::<NativeStyles>() else {
            return StyleRefinement::default();
        };
        let part = states[0];
        let mut target = Target::new(
            if part == "input" { "input" } else { "div" },
            &id,
            &[
                &format!("fs-select-module-{part}"),
                &format!("mantine-Select-{part}"),
            ],
        );
        target
            .attributes
            .insert("data-variant".into(), "default".into());
        if states.contains(&"clearable") {
            target.classes.push("fs-select-module-clearable".into());
        }
        if states.contains(&"selected") {
            target
                .attributes
                .insert("data-combobox-selected".into(), String::new());
        }
        let mut parents = vec![Target::new(
            "div",
            "",
            &["fs-select-module-root", "mantine-Select-root"],
        )];
        if part == "option" {
            parents.push(Target::new(
                "div",
                "",
                &["fs-select-module-dropdown", "mantine-Select-dropdown"],
            ));
        }
        styles.style_in(
            target_with_states(target, states),
            parents,
            f32::from(cx.theme().font_size),
        )
    }))
}

pub(crate) fn select_stylesheet() -> String {
    // Mantine's default small input/option geometry precedes the original wrapper.
    format!(
        r#"
        .mantine-Select-input {{ height: 2.25rem; padding: 0 0.75rem; font-size: var(--theme-font-size-sm); border-radius: var(--theme-radius-sm); }}
        .mantine-Select-dropdown {{ font-size: var(--theme-font-size-md); border-radius: var(--theme-radius-sm); padding-right: 0; }}
        .mantine-Select-option {{ background: transparent; color: inherit; font-size: var(--theme-font-size-sm); line-height: 1.125rem; border-radius: var(--theme-radius-sm); }}
        .mantine-Select-option:where([data-combobox-selected]) {{ background: var(--mantine-primary-color-filled); color: white; }}
        {}
        .fs-select-module-dropdown {{ padding-right: 0; }}
    "#,
        include_str!("../assets/select-paint.css")
    )
}
fn state_style(target: Target) -> gpui_component::StateStyle {
    state_style_in(target, None)
}
fn state_style_in(target: Target, parent: Option<Target>) -> gpui_component::StateStyle {
    state_style_path(target, parent.into_iter().collect())
}
fn target_with_states(mut target: Target, states: &[&str]) -> Target {
    if target
        .classes
        .iter()
        .any(|class| class == "mantine-Button-root")
        && let Some(variant) = states
            .iter()
            .find_map(|state| state.strip_prefix("variant-"))
    {
        target
            .attributes
            .insert("data-variant".into(), variant.into());
    }
    if states.contains(&"native-button-defaults") {
        target
            .attributes
            .insert("data-native-button-defaults".into(), String::new());
    }
    if let Some(size) = states.iter().find_map(|state| state.strip_prefix("size-")) {
        target.attributes.insert("data-size".into(), size.into());
    }
    if states.contains(&"with-left-section") {
        target
            .attributes
            .insert("data-with-left-section".into(), String::new());
    }
    target.states = states.iter().map(|state| (*state).into()).collect();
    for state in ["disabled", "checked"] {
        if states.contains(&state) {
            target.attributes.insert(state.into(), String::new());
            target
                .attributes
                .insert(format!("data-{state}"), "true".into());
        }
    }
    if states.contains(&"loading") {
        target.states.push("disabled".into());
        target.attributes.insert("disabled".into(), String::new());
        target
            .attributes
            .insert("data-loading".into(), "true".into());
    }
    if states.contains(&"selected") {
        target
            .attributes
            .insert("data-highlighted".into(), String::new());
        target
            .attributes
            .insert("data-hovered".into(), "true".into());
    }
    if states.contains(&"disabled") {
        target.classes.extend([
            "fs-context-menu-module-disabled".into(),
            "fs-dropdown-menu-module-disabled".into(),
        ]);
    }
    if states.contains(&"checked") {
        target.classes.extend([
            "fs-context-menu-module-selected".into(),
            "fs-dropdown-menu-module-selected".into(),
        ]);
    }
    target
}
pub(crate) fn state_style_path(target: Target, parents: Vec<Target>) -> gpui_component::StateStyle {
    std::rc::Rc::new(move |states, cx| {
        let Some(styles) = cx.try_global::<NativeStyles>() else {
            return StyleRefinement::default();
        };
        let mut target = target_with_states(target.clone(), states);
        let mut parent = parents.clone();
        if states.contains(&"context-menu") || states.contains(&"dropdown-menu") {
            let context = states.contains(&"context-menu");
            for target in std::iter::once(&mut target).chain(parent.iter_mut()) {
                target.classes.retain(|class| {
                    if context {
                        !class.starts_with("fs-dropdown-menu-module-")
                            && !class.starts_with("mantine-Menu-")
                    } else {
                        !class.starts_with("fs-context-menu-module-")
                    }
                });
                let role = match target.id.as_str() {
                    "native-popup-menu" => "menu",
                    "native-menu-item" => "menuitem",
                    "native-menu-divider" => "separator",
                    _ => "presentation",
                };
                target.attributes.insert("role".into(), role.into());
                if context && target.id == "native-menu-item" {
                    target.tag = "div".into();
                    if states.contains(&"disabled") {
                        target
                            .attributes
                            .insert("data-disabled".into(), String::new());
                    }
                }
            }
        }
        styles.style_in(target, parent, f32::from(cx.theme().font_size))
    })
}
pub(crate) fn button_stylesheet() -> String {
    // Mantine paint defaults precede the application's actual wrapper rules.
    // Geometry stays in Button so explicit builder dimensions retain precedence.
    // Selectable native choices also use this button. Keep their selected paint
    // until their corresponding original segmented/list controls are applied.
    let paint = include_str!("../assets/button-paint.css").replace(
        ".fs-button-module-root",
        ".fs-button-module-root:where(:not([data-checked='true']))",
    );
    format!(
        r#".mantine-Button-root:where(:not([data-checked='true'])):where(:not([data-variant='custom'])) {{
            background: var(--mantine-primary-color-filled);
            color: var(--mantine-primary-color-contrast);
        }}
        .mantine-Button-root:where(:not([data-checked='true'])):where([data-variant='outline'],[data-variant='transparent']) {{ background: transparent; }}
        .mantine-Button-root:where(:not([data-checked='true'])):where([data-variant='outline']):where(:not([data-loading='true'],[data-disabled='true'])):hover {{ background: var(--mantine-color-primary-outline-hover); }}
        .mantine-Button-root:where(:not([data-checked='true'])):where([data-disabled='true']):where(:not([data-loading='true'])) {{
            background: var(--mantine-color-disabled);
            color: var(--mantine-color-disabled-color);
        }}
        .mantine-Button-root:where([data-disabled='true']) {{ opacity: 0.6; }}
        {paint}
        .fs-play-button-module-wide-text-button {{ border-radius: var(--theme-radius-xl); }}
        .fs-play-button-module-wide-text-button.fs-play-button-module-unthemed {{
            background: white; color: black;
        }}
        .fs-play-button-module-wide-text-button-label {{
            font-size: var(--theme-font-size-md); font-weight: 600;
        }}
        .fs-play-button-module-wide-text-button.fs-play-button-module-unthemed:is(:hover,:active,:focus-visible) {{
            background: #ccc;
        }}
        [data-mantine-color-scheme='light'] .fs-play-button-module-wide-text-button.fs-play-button-module-unthemed {{
            background: black; color: white;
        }}
        [data-mantine-color-scheme='light'] .fs-play-button-module-wide-text-button.fs-play-button-module-unthemed:is(:hover,:active,:focus-visible) {{
            background: #1a1a1a;
        }}
        {}"#,
        include_str!("../assets/button-layout.css")
    )
}

pub(crate) fn modal_stylesheet() -> String {
    // Native headings already disable text selection.
    let mut css = include_str!("../assets/styles/modal.css").replace("    user-select: none;", "");
    for part in ["header", "content", "close", "body"] {
        css = css.replace(&format!(".{part}"), &format!(".fs-modal-module-{part}"));
    }
    css.push_str(
        "\n.mantine-Modal-overlay { background: rgb(0 0 0 / 50%); backdrop-filter: blur(1px); }",
    );
    css
}

fn modal_root_context(context: &[Target]) -> Vec<Target> {
    context.iter().take(1).cloned().collect()
}

fn modal_target(part: &str) -> Target {
    Target::new(
        if part == "title" {
            "h2"
        } else if part == "close" {
            "button"
        } else {
            "div"
        },
        &format!("native-modal-{part}"),
        &[
            &format!("fs-modal-module-{part}"),
            &format!("mantine-Modal-{part}"),
        ],
    )
}

#[derive(Clone, Copy)]
enum StyleScope {
    ModalBody,
    ExpandedPlayer,
}
impl StyleScope {
    fn enter(self, cx: &mut App) -> Option<Vec<Target>> {
        if !cx.has_global::<NativeStyles>() {
            return None;
        }
        let styles = cx.global_mut::<NativeStyles>();
        let previous = std::mem::take(&mut styles.context);
        styles.context = match self {
            Self::ModalBody => modal_root_context(&previous)
                .into_iter()
                .chain([
                    modal_target("root"),
                    modal_target("inner"),
                    modal_target("content"),
                    modal_target("body"),
                ])
                .collect(),
            Self::ExpandedPlayer => previous
                .iter()
                .cloned()
                .chain([Target::new(
                    "div",
                    "expanded-player",
                    &["fs-full-screen-player-module-container"],
                )])
                .collect(),
        };
        Some(previous)
    }

    fn restore(previous: Option<Vec<Target>>, cx: &mut App) {
        if let Some(previous) = previous {
            cx.global_mut::<NativeStyles>().context = previous;
        }
    }
}

pub(crate) fn expanded_player_scope<T: 'static>(
    cx: &mut Context<T>,
    render: impl FnOnce(&mut Context<T>) -> AnyElement,
) -> AnyElement {
    let previous = StyleScope::ExpandedPlayer.enter(cx);
    let element = render(cx);
    StyleScope::restore(previous, cx);
    ScopedStyles(element, StyleScope::ExpandedPlayer).into_any_element()
}

// Lazy controls and canvas children resolve CSS in different rendering phases.
// Retain each subtree's ancestors through those phases without styling siblings.
struct ScopedStyles(AnyElement, StyleScope);
impl IntoElement for ScopedStyles {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ScopedStyles {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let previous = self.1.enter(cx);
        let layout = self.0.request_layout(window, cx);
        StyleScope::restore(previous, cx);
        (layout, ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let previous = self.1.enter(cx);
        self.0.prepaint(window, cx);
        StyleScope::restore(previous, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let previous = self.1.enter(cx);
        self.0.paint(window, cx);
        StyleScope::restore(previous, cx);
    }
}

pub(crate) fn install_dialog_styles(cx: &mut App) {
    use gpui_component::{Icon, Sizable, dialog::DialogStyles};
    if cx.has_global::<DialogStyles>() {
        return;
    }
    let style = |part, parents: &[&str]| {
        state_style_path(
            modal_target(part),
            parents.iter().map(|part| modal_target(part)).collect(),
        )
    };
    cx.set_global(DialogStyles {
        overlay: style("overlay", &["root"]),
        content: style("content", &["root", "inner"]),
        header: style("header", &["root", "inner", "content"]),
        title: style("title", &["root", "inner", "content", "header"]),
        body: style("body", &["root", "inner", "content"]),
        close: style("close", &["root", "inner", "content", "header"]),
        close_icon: std::rc::Rc::new(|cx| {
            Icon::default()
                .path("native-icons/window-close.svg")
                .with_size(cx.theme().font_size * 1.5)
        }),
        close_label: std::rc::Rc::new(|cx| crate::appearance::text("common.close", cx).into()),
        wrap_body: std::rc::Rc::new(|body| {
            ScopedStyles(body, StyleScope::ModalBody).into_any_element()
        }),
    });
}

fn install_popup_styles(cx: &mut App) {
    use gpui_component::menu::PopupMenuStyles;
    if cx.has_global::<PopupMenuStyles>() {
        return;
    }
    let dropdown = Target::new(
        "div",
        "native-popup-menu",
        &[
            "fs-context-menu-module-content",
            "fs-dropdown-menu-module-menu-dropdown",
            "mantine-Menu-dropdown",
        ],
    );
    let child = |tag, id, classes: &[&str]| {
        state_style_in(Target::new(tag, id, classes), Some(dropdown.clone()))
    };
    cx.set_global(PopupMenuStyles {
        item: child(
            "button",
            "native-menu-item",
            &[
                "fs-context-menu-module-item",
                "fs-dropdown-menu-module-menu-item",
                "mantine-Menu-item",
            ],
        ),
        divider: child(
            "div",
            "native-menu-divider",
            &[
                "fs-context-menu-module-divider",
                "fs-dropdown-menu-module-menu-divider",
                "mantine-Menu-divider",
            ],
        ),
        label: child(
            "div",
            "native-menu-label",
            &[
                "fs-dropdown-menu-module-menu-label",
                "fs-dropdown-menu-module-menu-item-label",
                "mantine-Menu-label",
            ],
        ),
        dropdown: state_style(dropdown),
    });
}
pub fn number_input(
    state: &Entity<gpui_component::input::InputState>,
    cx: &App,
) -> gpui_component::input::NumberInput {
    apply(
        gpui_component::input::NumberInput::new(state)
            .input_style(
                div()
                    .h(rems(2.25))
                    .text_size(rems(13. / 16.))
                    .style()
                    .clone(),
            )
            .step_labels(
                format!(
                    "{}: {}",
                    crate::appearance::text("Decrease", cx),
                    state.read(cx).accessible_name(cx)
                ),
                format!(
                    "{}: {}",
                    crate::appearance::text("Increase", cx),
                    state.read(cx).accessible_name(cx)
                ),
            )
            .state_style(state_style(Target::new(
                "input",
                &state.entity_id().to_string(),
                &["fs-number-input-module-input", "mantine-Input-input"],
            ))),
        "input",
        &state.entity_id().to_string(),
        &["fs-number-input-module-input", "mantine-Input-input"],
        None,
        cx,
    )
}
impl Feishin {
    pub fn sync_css_environment(&self, window: &Window, cx: &mut Context<Self>) {
        if !cx.has_global::<NativeStyles>() {
            return;
        }
        let styles = cx.global_mut::<NativeStyles>();
        let zoom = self.state.settings.zoom.max(0.1);
        let size = window.viewport_size();
        let media = feishin_core::styles::MediaEnvironment {
            width: f32::from(size.width) / zoom,
            height: f32::from(size.height) / zoom,
            resolution: window.scale_factor() * zoom,
            dark: styles.palette.mode == "dark",
            hover: true,
        };
        if styles.media != media {
            styles.media = media;
            styles.display_scale = window.scale_factor();
            styles.cache.borrow_mut().clear();
            styles.motion_cache.borrow_mut().clear();
        }
    }
    pub fn sync_css_file(&mut self) {
        let path = self.data_dir.join("custom.css");
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.len() <= 1_000_000 => {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let signature = format!("{:x}", feishin_core::api::cover_hash(&content));
                    if self.css_file_signature.as_ref() != Some(&signature) {
                        self.css_file_signature = Some(signature);
                        if content != self.state.settings.css.content {
                            self.state.settings.css.content = content;
                            self.save();
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if self.css_file_signature.is_none() && !self.state.settings.css.content.is_empty()
                {
                    if feishin_core::storage::atomic_write(
                        &path,
                        self.state.settings.css.content.as_bytes(),
                    )
                    .is_ok()
                    {
                        self.css_file_signature = Some(format!(
                            "{:x}",
                            feishin_core::api::cover_hash(&self.state.settings.css.content)
                        ));
                    }
                } else if self.css_file_signature.as_deref() != Some("missing") {
                    if self.css_file_signature.is_some() {
                        self.state.settings.css.content.clear();
                        self.save();
                    }
                    self.css_file_signature = Some("missing".into());
                }
            }
            _ => {}
        }
    }
    pub fn sync_style_context(&self, window: &Window, cx: &mut Context<Self>) {
        install_popup_styles(cx);
        install_dialog_styles(cx);
        if !cx.has_global::<NativeStyles>() {
            return;
        }
        let styles = cx.global_mut::<NativeStyles>();
        let mut root = Target::new("body", "native-root", &[]);
        if self.full_player {
            root.children.push(Target::new(
                "div",
                "expanded-player",
                &["fs-full-screen-player-module-container"],
            ));
        }
        if !window.is_fullscreen()
            && matches!(self.window_bar_style_active.as_str(), "macos" | "windows")
        {
            root.children.push(Target::new(
                "div",
                "native-window-bar",
                &["fs-window-bar-module-window-bar"],
            ));
        }
        root.attributes.insert(
            "data-mantine-color-scheme".into(),
            styles.palette.mode.clone(),
        );
        root.attributes.insert(
            "dir".into(),
            if feishin_core::i18n::rtl(&self.state.settings.language) {
                "rtl"
            } else {
                "ltr"
            }
            .into(),
        );
        let context = vec![
            root,
            Target::new(
                "div",
                "themed-content",
                &["fs-main-content-module-main-content-container"],
            ),
        ];
        if format!("{context:?}") != format!("{:?}", styles.context) {
            styles.context = context;
            styles.cache.borrow_mut().clear();
            styles.motion_cache.borrow_mut().clear();
        }
    }
    pub(crate) fn save_custom_css(
        &mut self,
        content: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if content.len() > 1_000_000 {
            self.message = "Custom CSS is too large".into();
            cx.notify();
            return;
        }
        match feishin_core::storage::atomic_write(
            &self.data_dir.join("custom.css"),
            content.as_bytes(),
        ) {
            Ok(()) => {
                self.state.settings.css.content = content;
                self.save();
                self.apply_theme(window, cx);
            }
            Err(error) => {
                log::warn!("Unable to save custom CSS: {error}");
                self.message = error.to_string();
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn image_fit_pixels_match_original_css(cx: &mut TestAppContext) {
        struct Probe(std::sync::Arc<gpui::RenderImage>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .relative()
                    .size(px(128.))
                    .bg(rgb(0))
                    .child(super::apply_in(
                        gpui::img(self.0.clone())
                            .absolute()
                            .left(px(24.))
                            .top(px(24.))
                            .size(px(80.))
                            .object_fit(gpui::ObjectFit::Contain),
                        Target::new("img", "probe", &[]),
                        [Target::new("body", "", &[])],
                        cx,
                    ))
            }
        }
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/image-fit-linux-reference.json")
        } else {
            include_str!("../assets/image-fit-reference.json")
        })
        .unwrap();
        cx.update(gpui_component::init);
        for case in reference["cases"].as_array().unwrap() {
            cx.update(|cx| {
                let styles = NativeStyles::new(
                    Stylesheet::parse([case["css"].as_str().unwrap()]).unwrap(),
                    appearance::Palette::default(),
                );
                assert!(
                    styles.warnings.is_empty(),
                    "{}: {:?}",
                    case["value"],
                    styles.warnings
                );
                cx.set_global(styles);
            });
            let width = case["size"][0].as_u64().unwrap() as u32;
            let height = case["size"][1].as_u64().unwrap() as u32;
            let source = image::RgbaImage::from_fn(width, height, |x, y| {
                let color = [
                    0xee3344u32,
                    0x33dd55,
                    0x4466ee,
                    0xeedd33,
                    0xdd33ee,
                    0x33ddee,
                    0xee8833,
                    0x888888,
                ][(x / (width / 4)) as usize + 4 * usize::from(y >= height / 2)];
                // GPUI's image atlas consumes BGRA source frames.
                image::Rgba([color as u8, (color >> 8) as u8, (color >> 16) as u8, 255])
            });
            let source =
                std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(source)]));
            let (_, cx) = cx.add_window_view(|_, _| Probe(source));
            cx.run_until_parked();
            let (actual, scale) = cx.update(|window, _| {
                (
                    window
                        .rasterized_scene_for_test(size(px(128.), px(128.)))
                        .unwrap(),
                    window.scale_factor(),
                )
            });
            let stride = (128. * scale).round() as usize;
            let expected = case["pixels"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>();
            let mut index = 0;
            for y in reference["coordinates"].as_array().unwrap() {
                for x in reference["coordinates"].as_array().unwrap() {
                    let x = ((x.as_f64().unwrap() as f32 + 0.5) * scale).floor() as usize;
                    let y = ((y.as_f64().unwrap() as f32 + 0.5) * scale).floor() as usize;
                    let pixel = &actual[(y * stride + x) * 4..][..3];
                    let radius = case["radius"].as_f64().unwrap() as f32;
                    let dx = ((x as f32 + 0.5) / scale - 64.).abs() - (40. - radius);
                    let dy = ((y as f32 + 0.5) / scale - 64.).abs() - (40. - radius);
                    let edge = dx.max(0.).hypot(dy.max(0.)) + dx.max(dy).min(0.) - radius;
                    // Original Linux Chromium differs at fractional rounded edges.
                    // Limit that allowance to a one-device-pixel corner perimeter.
                    let tolerance = if cfg!(target_os = "linux")
                        && radius > 0.
                        && dx > 0.
                        && dy > 0.
                        && edge.abs() <= 1. / scale
                    {
                        10
                    } else {
                        3
                    };
                    assert!(
                        pixel
                            .iter()
                            .zip(&expected[index..index + 3])
                            .all(|(a, b)| a.abs_diff(*b) <= tolerance),
                        "{} {:?}, radius {}, ({x},{y}): {:?} != {:?}",
                        case["value"],
                        case["size"],
                        case["radius"],
                        pixel,
                        &expected[index..index + 3]
                    );
                    index += 3;
                }
            }
        }
    }

    #[gpui::test]
    fn wrapped_selects_match_original_theme_and_interaction_paint(cx: &mut TestAppContext) {
        use gpui_component::{Root, select::SelectState};
        struct Probe {
            select: Entity<SelectState<Vec<String>>>,
            disabled: bool,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().p_4().child(
                    div()
                        .w(px(180.))
                        .h(rems(2.25))
                        .debug_selector(|| "wrapped-select".into())
                        .child(
                            super::select(&self.select)
                                .placeholder("Select preset...")
                                .cleanable(true)
                                .disabled(self.disabled),
                        ),
                )
            }
        }
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/select-styles-reference.json")).unwrap();
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        for case in reference["cases"].as_array().unwrap() {
            let rem = px(case["rem"].as_f64().unwrap() as f32);
            cx.update(|cx| {
                gpui_component::Theme::global_mut(cx).font_size = rem;
                let palette = appearance::Palette::default();
                let mut variables = appearance::css_variables(&palette);
                variables.extend(
                    serde_json::from_value::<BTreeMap<String, String>>(
                        reference["variables"].clone(),
                    )
                    .unwrap(),
                );
                variables.insert("--mantine-primary-color-filled".into(), "#0088cc".into());
                let custom = if case["custom"] == true {
                    reference["custom"].as_str().unwrap()
                } else {
                    ""
                };
                cx.set_global(
                    NativeStyles::new(
                        Stylesheet::parse([super::select_stylesheet().as_str(), custom]).unwrap(),
                        palette,
                    )
                    .with_variables(variables),
                );
            });
            let (_, cx) = cx.add_window_view(|window, cx| {
                window.set_rem_size(rem);
                window.activate_window();
                window.activate_accessibility_for_test();
                let select = cx.new(|cx| {
                    SelectState::new(
                        vec!["Bass".to_owned(), "Treble".to_owned()],
                        None,
                        window,
                        cx,
                    )
                });
                if case["state"] == "focus" {
                    select.update(cx, |select, cx| select.focus(window, cx));
                }
                let probe = cx.new(|_| Probe {
                    select,
                    disabled: case["disabled"] == true,
                });
                Root::new(probe, window, cx)
            });
            cx.run_until_parked();
            let bounds = cx.debug_bounds("wrapped-select").unwrap();
            if case["open"] == true {
                cx.simulate_click(bounds.center(), Modifiers::none());
                cx.run_until_parked();
                let (option, scale) = cx.update(|window, _| {
                    let option = window
                        .accessibility_tree_for_test()
                        .unwrap()
                        .nodes
                        .iter()
                        .find(|(_, node)| {
                            node.role() == Role::ListBoxOption && node.label() == Some("Bass")
                        })
                        .and_then(|(_, node)| node.bounds())
                        .unwrap();
                    (option, window.scale_factor())
                });
                assert!(
                    ((option.y1 - option.y0) / f64::from(scale)
                        - case["option"]["height"].as_f64().unwrap())
                    .abs()
                        <= 1.,
                    "Option height: {case}; {option:?}"
                );
                cx.simulate_mouse_move(
                    point(
                        px((option.x0 + option.x1) as f32 / (2. * scale)),
                        px((option.y0 + option.y1) as f32 / (2. * scale)),
                    ),
                    None,
                    Modifiers::none(),
                );
                cx.run_until_parked();
            }
            cx.refresh().unwrap();
            let same_color = |actual: Hsla, text: &str, opacity: f32| {
                let mut expected = super::color(text, &appearance::Palette::default())
                    .unwrap()
                    .to_rgb();
                expected.a *= opacity;
                let actual = actual.to_rgb();
                (actual.a - expected.a).abs() < 0.00001
                    && (actual.a == 0.
                        || (actual.r - expected.r)
                            .abs()
                            .max((actual.g - expected.g).abs())
                            .max((actual.b - expected.b).abs())
                            < 0.00001)
            };
            let paint = cx.update(|window, _| window.painted_quad_opacities_for_test());
            let input = &case["input"];
            assert_eq!(
                f64::from(bounds.size.height),
                input["height"].as_f64().unwrap(),
                "{case}"
            );
            assert!(
                paint.iter().any(|(actual, color)| *actual == bounds
                    && same_color(
                        *color,
                        input["background"].as_str().unwrap(),
                        input["opacity"].as_f64().unwrap() as f32
                    )),
                "Input background: {case}; {paint:?}"
            );
            let boxes = cx.update(|window, _| window.painted_boxes_for_test());
            assert!(
                boxes
                    .iter()
                    .any(|(actual, widths, radii, border)| *actual == bounds
                        && (border.a == 0. || widths.top == px(1.))
                        && (f64::from(radii.top_left) - input["radius"].as_f64().unwrap()).abs()
                            <= 0.01
                        && same_color(*border, input["border"].as_str().unwrap(), 1.)),
                "Input border: {case}; {boxes:?}"
            );
            if case["open"] == true {
                for part in ["dropdown", "option"] {
                    assert!(
                        paint
                            .iter()
                            .any(|(actual, color)| actual.top() > bounds.bottom()
                                && same_color(
                                    *color,
                                    case[part]["background"].as_str().unwrap(),
                                    1.
                                )),
                        "{part}: {case}; {paint:?}"
                    );
                }
            }
        }
    }

    #[gpui::test]
    fn wrapped_buttons_match_original_pointer_and_keyboard_focus(cx: &mut TestAppContext) {
        use gpui_component::{
            Disableable, Root,
            button::ButtonVariants,
            input::{InputEvent, InputState},
        };
        use std::rc::Rc;
        struct Probe {
            input: Entity<InputState>,
            button: FocusHandle,
            blurred: bool,
            events: Rc<RefCell<Vec<&'static str>>>,
            _subscriptions: Vec<Subscription>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .v_flex()
                    .gap_4()
                    .p_8()
                    .child(super::input(&self.input, cx).w(px(180.)))
                    .child(
                        super::button("focus-button", cx)
                            .primary()
                            .label("Action")
                            .focus_handle(&self.button)
                            .debug_selector(|| "focus-button".into())
                            .on_click(cx.listener(|view, _, _, _| {
                                assert!(view.blurred, "Input blur must commit before activation");
                                view.events.borrow_mut().push("click");
                            })),
                    )
                    .child(
                        super::button("disabled-button", cx)
                            .label("Disabled")
                            .disabled(true)
                            .debug_selector(|| "disabled-button".into()),
                    )
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let capture = Rc::new(RefCell::new(None));
        let captured = capture.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            let probe = cx.new(|cx| {
                let input = cx.new(|cx| InputState::new(window, cx).aria_label("Draft"));
                let button = cx.focus_handle();
                let events = Rc::new(RefCell::new(Vec::new()));
                let blur_events = events.clone();
                let focus_events = events.clone();
                let input_focus = input.read(cx).focus_handle(cx);
                let subscriptions = vec![
                    cx.on_blur(&input_focus, window, move |_, _, _| {
                        blur_events.borrow_mut().push("blur")
                    }),
                    cx.on_focus(&button, window, move |_, _, _| {
                        focus_events.borrow_mut().push("focus")
                    }),
                    cx.subscribe_in(
                        &input,
                        window,
                        |view: &mut Probe, _, event: &InputEvent, _, _| {
                            if matches!(event, InputEvent::Blur) {
                                view.blurred = true;
                            }
                        },
                    ),
                ];
                Probe {
                    input,
                    button,
                    blurred: false,
                    events,
                    _subscriptions: subscriptions,
                }
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = capture.borrow_mut().take().unwrap();
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/button-focus-reference.json")).unwrap();
        for case in cases {
            let name = case["id"].as_str().unwrap();
            if !matches!(name, "Enter" | "Space") {
                cx.update(|window, cx| {
                    let input = probe.read(cx).input.clone();
                    input.update(cx, |input, cx| input.focus(window, cx));
                    probe.read(cx).events.borrow_mut().clear();
                    probe.update(cx, |view, _| view.blurred = false);
                });
                cx.run_until_parked();
            }
            match name {
                "button" | "disabled" => {
                    let bounds = cx
                        .debug_bounds(if name == "button" {
                            "focus-button"
                        } else {
                            "disabled-button"
                        })
                        .unwrap();
                    cx.simulate_click(bounds.center(), Modifiers::none());
                }
                "Tab" => cx.simulate_keystrokes("tab"),
                "Enter" | "Space" => {
                    let key = name.to_lowercase();
                    cx.simulate_keystrokes(&key);
                    cx.simulate_event(KeyUpEvent {
                        keystroke: Keystroke::parse(&key).unwrap(),
                    });
                }
                _ => unreachable!(),
            }
            cx.run_until_parked();
            cx.update(|window, cx| {
                let probe = probe.read(cx);
                assert_eq!(
                    probe.button.is_focused(window),
                    case["active"] == "button",
                    "{name}"
                );
                assert_eq!(
                    probe.button.is_focused(window) && window.is_focus_visible(),
                    case["focusVisible"].as_bool().unwrap(),
                    "{name} focus-visible"
                );
                assert!(
                    !probe.input.read(cx).focus_handle(cx).is_focused(window),
                    "{name} retained text focus"
                );
                assert_eq!(
                    serde_json::to_value(probe.events.borrow().clone()).unwrap(),
                    case["events"],
                    "{name}"
                );
            });
        }
    }

    #[gpui::test]
    fn wrapped_buttons_match_original_styles_and_geometry(cx: &mut TestAppContext) {
        use gpui_component::{
            Disableable, Icon, IconName, Root, Sizable, Size, button::ButtonVariants,
        };
        struct Probe {
            variant: String,
            size: Option<String>,
            icon: bool,
            disabled: bool,
            focus: FocusHandle,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let button = super::button("variant-button", cx)
                    .child(
                        div()
                            .debug_selector(|| "button-content".into())
                            .child("Button"),
                    )
                    .when_some(self.size.as_ref(), |button, size| {
                        button
                            .with_size(match size.trim_start_matches("compact-") {
                                "xs" => Size::XSmall,
                                "sm" => Size::Small,
                                "md" => Size::Medium,
                                _ => Size::Large,
                            })
                            .when(size.starts_with("compact-"), |button| button.compact())
                    })
                    .when(self.icon, |button| {
                        button.icon(Icon::new(IconName::Check).with_size(px(16.)))
                    })
                    .disabled(self.disabled)
                    .on_click(|_, _, _| {})
                    .debug_selector(|| "variant-button".into());
                div()
                    .flex()
                    .items_start()
                    .track_focus(&self.focus)
                    .p_8()
                    .child(match self.variant.as_str() {
                        "filled" => button.primary(),
                        "subtle" => button.ghost(),
                        "outline" => button.outline(),
                        "transparent" => button.text(),
                        "state-error" => button.danger(),
                        "state-info" => button.info(),
                        "state-success" => button.success(),
                        "state-warning" => button.warning(),
                        _ => button,
                    })
            }
        }
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/button-styles-reference.json")).unwrap();
        cx.set_assets(crate::icons::Assets);
        cx.use_platform_text_system();
        cx.update(gpui_component::init);
        for case in reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .chain(reference["sizes"].as_array().unwrap().iter())
            .filter(|case| case["variant"] != "secondary")
        {
            cx.update(|cx| {
                gpui_component::Theme::global_mut(cx).font_size =
                    px(case["rem"].as_f64().unwrap() as f32);
                gpui_component::Theme::global_mut(cx).primary = gpui::rgb(0x0088cc).into();
                let palette = appearance::Palette {
                    mode: case["mode"].as_str().unwrap().into(),
                    ..Default::default()
                };
                let mut variables = appearance::css_variables(&palette);
                variables.extend(
                    serde_json::from_value::<BTreeMap<String, String>>(
                        reference["variables"].clone(),
                    )
                    .unwrap(),
                );
                variables.insert("--mantine-primary-color-filled".into(), "#0088cc".into());
                variables.insert("--mantine-primary-color-contrast".into(), "#ffffff".into());
                variables.insert(
                    "--mantine-color-primary-outline-hover".into(),
                    "rgba(0,136,204,.05)".into(),
                );
                let custom = match case["theme"].as_str().unwrap() {
                    "custom" => reference["custom"].as_str().unwrap(),
                    "glassy" => {
                        include_str!("../assets/styles/glassy-dark.css")
                    }
                    _ => "",
                };
                cx.set_global(
                    NativeStyles::new(
                        Stylesheet::parse([super::button_stylesheet().as_str(), custom]).unwrap(),
                        palette,
                    )
                    .with_variables(variables),
                );
            });
            let (_, cx) = cx.add_window_view(|window, cx| {
                let focus = cx.focus_handle();
                focus.focus(window);
                let probe = cx.new(|_| Probe {
                    variant: case["variant"].as_str().unwrap().into(),
                    size: case["size"].as_str().map(str::to_owned),
                    icon: case["icon"].as_bool().unwrap_or(false),
                    disabled: case["disabled"].as_bool().unwrap(),
                    focus,
                });
                Root::new(probe, window, cx)
            });
            cx.run_until_parked();
            let bounds = cx.debug_bounds("variant-button").unwrap();
            if case["state"] == "hover" || case["state"] == "active" {
                cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
                if case["state"] == "active" {
                    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
                }
            } else if case["state"] == "focus" {
                cx.simulate_keystrokes("tab");
            }
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.refresh().unwrap();
            cx.run_until_parked();
            let expected_color = |key: &str| {
                let mut color =
                    super::color(case[key].as_str().unwrap(), &appearance::Palette::default())
                        .unwrap();
                color.a *= case["opacity"].as_f64().unwrap() as f32;
                color
            };
            let close_color = |a: Hsla, b: Hsla| {
                let a = a.to_rgb();
                let b = b.to_rgb();
                (a.a - b.a).abs() < 0.00001
                    && (a.a == 0.
                        || (a.r - b.r)
                            .abs()
                            .max((a.g - b.g).abs())
                            .max((a.b - b.b).abs())
                            < 0.00001)
            };
            let paint = cx.update(|window, _| window.painted_quad_opacities_for_test());
            let background = expected_color("background");
            assert!(
                if background.a == 0. {
                    paint
                        .iter()
                        .all(|(actual, color)| *actual != bounds || color.a == 0.)
                } else {
                    paint
                        .iter()
                        .any(|(actual, color)| *actual == bounds && close_color(*color, background))
                },
                "background: {case}; {paint:?}"
            );
            let boxes = cx.update(|window, _| window.painted_boxes_for_test());
            assert!(
                (background.a == 0. && expected_color("border").a == 0.)
                    || boxes
                        .iter()
                        .any(|(actual, borders, corners, color)| *actual == bounds
                            && corners.top_left == px(case["radius"].as_f64().unwrap() as f32)
                            && (color.a == 0.
                                || borders.top
                                    == px(case["borderWidth"].as_f64().unwrap() as f32))
                            && close_color(
                                *color,
                                super::color(
                                    case["border"].as_str().unwrap(),
                                    &appearance::Palette::default()
                                )
                                .unwrap()
                            )),
                "border/radius: {case}; {boxes:?}"
            );
            assert!(
                cx.update(|window, _| window.painted_glyphs_with_opacity_for_test())
                    .iter()
                    .any(|(_, _, color)| close_color(*color, expected_color("color"))),
                "text: {case}; {:?}",
                cx.update(|window, _| window.painted_glyphs_with_opacity_for_test())
            );
            assert_eq!(
                bounds.size.height,
                px(case["height"].as_f64().unwrap() as f32),
                "height: {case}"
            );
            let content = cx.debug_bounds("button-content").unwrap();
            let padding = case["contentOffset"]
                .as_f64()
                .map(|v| v as f32)
                .unwrap_or_else(|| {
                    case["padding"][3].as_f64().unwrap() as f32
                        + case["borderWidth"].as_f64().unwrap() as f32
                });
            assert!(
                (f32::from(content.left() - bounds.left()) - padding).abs() < 1.,
                "padding: {case}; {bounds:?} {content:?}"
            );
            if case["outlineStyle"] == "solid" {
                let expected = super::color(
                    case["outline"].as_str().unwrap(),
                    &appearance::Palette::default(),
                )
                .unwrap();
                let width = px(case["outlineWidth"].as_f64().unwrap() as f32);
                let outer =
                    bounds.dilate(width + px(case["outlineOffset"].as_f64().unwrap() as f32));
                assert!(
                    boxes
                        .iter()
                        .any(|(actual, borders, _, color)| *actual == outer
                            && borders.top == width
                            && close_color(*color, expected)),
                    "outline: {case}; {boxes:?}"
                );
            }
            cx.update(|window, _| window.remove_window());
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn modal_frame_matches_original_centering_bounds_fonts_and_direction(cx: &mut TestAppContext) {
        cx.set_assets(crate::icons::Assets);
        use gpui_component::{Root, WindowExt};
        struct Probe(bool);
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .size_full()
                    .text_direction(if self.0 {
                        TextDirection::Rtl
                    } else {
                        TextDirection::Ltr
                    })
                    .children(Root::render_dialog_layer(window, cx))
            }
        }
        cx.update(gpui_component::init);
        cx.update(super::install_dialog_styles);
        #[cfg(target_os = "macos")]
        let source = include_str!("../assets/modal-layout-darwin-reference.json");
        #[cfg(target_os = "linux")]
        let source = include_str!("../assets/modal-layout-linux-reference.json");
        let mut cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
        #[cfg(target_os = "macos")]
        let settings = include_str!("../assets/modal-settings-darwin-reference.json");
        #[cfg(target_os = "linux")]
        let settings = include_str!("../assets/modal-settings-linux-reference.json");
        cases.extend(serde_json::from_str::<Vec<serde_json::Value>>(settings).unwrap());
        for case in cases {
            let rem = case["rem"].as_f64().unwrap() as f32;
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let rtl = case["rtl"].as_bool().unwrap();
            cx.update(|cx| {
                gpui_component::Theme::global_mut(cx).font_size = px(rem * zoom);
                cx.set_global(
                    NativeStyles::new(
                        Stylesheet::parse([super::modal_stylesheet().as_str()]).unwrap(),
                        appearance::builtins()["defaultTheme"].clone(),
                    )
                    .with_zoom(zoom)
                    .with_font_metrics(
                        cx.text_system().clone(),
                        gpui::font("Inter"),
                        if cfg!(target_os = "macos") { 2. } else { 1. },
                    ),
                );
            });
            let (_, cx) = cx.add_window_view(|window, cx| {
                let probe = cx.new(|_| Probe(rtl));
                Root::new(probe, window, cx)
            });
            cx.simulate_resize(size(
                px(case["width"].as_f64().unwrap() as f32 * zoom),
                px(case["height"].as_f64().unwrap() as f32 * zoom),
            ));
            cx.update(|window, cx| {
                let settings = case["settings"].as_str().map(str::to_owned);
                let viewport_height = window.viewport_size().height;
                let title = !case["title"].is_null();
                let close = !case["close"].is_null();
                let height = case["contentHeight"].as_f64().unwrap() as f32 * zoom;
                window.open_dialog(cx, move |dialog, _, _| {
                    dialog
                        .width(rems(if settings.is_some() { 48.75 } else { 38.75 }))
                        .when(settings.is_some(), |dialog| dialog.overlay(false))
                        .when(settings.as_deref() == Some("visualizer"), |dialog| {
                            dialog
                                .max_w(px(1400. * zoom))
                                .h(viewport_height * 0.81)
                                .min_h(px(600. * zoom))
                        })
                        .when(settings.as_deref() == Some("export"), |dialog| {
                            dialog.body_height(px(600. * zoom))
                        })
                        .close_button(close)
                        .when(title, |dialog| dialog.title("Modal title"))
                        .child(
                            div()
                                .debug_selector(|| "modal-frame-child".into())
                                .h(px(height))
                                .flex_shrink_0()
                                .child("Body content"),
                        )
                });
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(310));
            cx.refresh().unwrap();
            cx.run_until_parked();
            for part in ["content", "header", "title", "close", "child"] {
                let selector = if part == "child" {
                    "modal-frame-child".into()
                } else {
                    format!("native-modal-{part}-0")
                };
                if case[part].is_null() {
                    continue;
                }
                let actual = cx.debug_bounds(&selector).unwrap();
                let expected = if part == "child" {
                    &case[part]
                } else {
                    &case[part]["bounds"]
                };
                let actual = [
                    actual.origin.x,
                    actual.origin.y,
                    actual.size.width,
                    actual.size.height,
                ]
                .map(f32::from);
                for (index, actual) in actual.into_iter().enumerate() {
                    let expected = expected[index].as_f64().unwrap() as f32 * zoom;
                    assert!(
                        (actual - expected).abs() <= 1.01,
                        "{part}[{index}] {actual} != {expected}: {case}"
                    );
                }
            }
            if case["contentHeight"] == 900 {
                let before = cx.debug_bounds("modal-frame-child").unwrap();
                let header = cx.debug_bounds("native-modal-header-0");
                let body = cx.debug_bounds("native-modal-body-0").unwrap();
                cx.simulate_event(ScrollWheelEvent {
                    position: body.center(),
                    delta: ScrollDelta::Pixels(point(px(0.), px(-100.))),
                    modifiers: Modifiers::none(),
                    touch_phase: TouchPhase::Moved,
                });
                cx.run_until_parked();
                assert!(
                    cx.debug_bounds("modal-frame-child").unwrap().top() < before.top(),
                    "Overflowing modal must scroll: {case}"
                );
                assert_eq!(cx.debug_bounds("native-modal-header-0"), header);
            }
            cx.update(|window, _| window.remove_window());
        }
    }

    #[gpui::test]
    fn modal_pop_matches_original_motion_and_transparent_overlay_dismissal(
        cx: &mut TestAppContext,
    ) {
        use gpui_component::{Root, WindowExt};
        use std::{cell::Cell, rc::Rc, time::Duration};
        struct Probe(FocusHandle);
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .size_full()
                    .track_focus(&self.0)
                    .children(Root::render_dialog_layer(window, cx))
            }
        }
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/modal-pop-transition-reference.json"
        ))
        .unwrap();
        cx.update(gpui_component::init);
        for zoom in [1., 1.25] {
            let allow = Rc::new(Cell::new(false));
            let outside = cx.update(|cx| cx.focus_handle());
            let (_, cx) = cx.add_window_view(|window, cx| {
                outside.focus(window);
                let probe = cx.new(|_| Probe(outside.clone()));
                Root::new(probe, window, cx)
            });
            cx.simulate_resize(size(px(1000.), px(800.)));
            cx.update(|window, cx| {
                let allow = allow.clone();
                window.open_dialog(cx, move |dialog, _, _| {
                    let allow = allow.clone();
                    dialog
                        .width(px(400.))
                        .bg(rgb(0x123456))
                        .close_button(false)
                        .overlay(false)
                        .pop_transition(px(10. * zoom))
                        .on_cancel(move |_, _, _| allow.get())
                        .child(div().h(px(80.)).flex_shrink_0())
                });
            });
            cx.run_until_parked();
            for phase in ["entrance", "exit"] {
                let mut previous = 0;
                for frame in reference[phase].as_array().unwrap() {
                    let elapsed = frame["elapsed"].as_u64().unwrap();
                    cx.executor()
                        .advance_clock(Duration::from_millis(elapsed - previous));
                    cx.refresh().unwrap();
                    cx.run_until_parked();
                    let paint = cx.update(|window, _| window.painted_quad_opacities_for_test());
                    let target: Hsla = rgb(0x123456).into();
                    let painted = paint.iter().find(|(_, color)| {
                        (color.h - target.h).abs() < 0.0001 && (color.s - target.s).abs() < 0.0001
                    });
                    let opacity = frame["opacity"].as_f64().unwrap() as f32;
                    assert!(
                        (painted.map_or(0., |(_, color)| color.a) - opacity).abs() < 0.00002,
                        "{phase} {frame}: {paint:?}"
                    );
                    if let Some((bounds, _)) = painted {
                        let transform: Vec<f32> = frame["transform"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("matrix(")
                            .trim_end_matches(')')
                            .split(',')
                            .map(|n| n.trim().parse().unwrap())
                            .collect();
                        let layout = cx.debug_bounds("native-modal-content-0").unwrap();
                        assert!(
                            (f32::from(bounds.size.width) - 400. * transform[0]).abs() < 0.01,
                            "{phase} {frame}: {bounds:?}"
                        );
                        assert!(
                            (f32::from(bounds.center().y) - (400. + transform[5] * zoom)).abs()
                                <= 1.01,
                            "{phase} {frame}: {bounds:?}, {layout:?}"
                        );
                    }
                    assert!(
                        paint
                            .iter()
                            .all(|(bounds, color)| bounds.size != size(px(1000.), px(800.))
                                || color.a == 0.
                                || *color == Hsla::white()),
                        "Transparent overlay painted: {paint:?}"
                    );
                    previous = elapsed;
                }
                if phase == "entrance" {
                    cx.simulate_click(point(px(500.), px(400.)), Modifiers::none());
                    cx.simulate_click(point(px(10.), px(10.)), Modifiers::none());
                    cx.simulate_keystrokes("escape");
                    cx.run_until_parked();
                    assert!(
                        cx.update(|window, cx| window.has_active_dialog(cx)),
                        "Cancel veto must retain the dialog"
                    );
                    allow.set(true);
                    cx.simulate_click(point(px(500.), px(400.)), Modifiers::none());
                    cx.run_until_parked();
                    assert!(
                        cx.update(|window, cx| window.has_active_dialog(cx)),
                        "An interior click must not dismiss the dialog"
                    );
                    cx.simulate_click(point(px(10.), px(10.)), Modifiers::none());
                    cx.run_until_parked();
                    assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
                    assert!(cx.update(|window, _| outside.is_focused(window)));
                }
            }
            cx.executor().advance_clock(Duration::from_millis(20));
            cx.refresh().unwrap();
            cx.run_until_parked();
            assert!(cx.update(|window, cx| Root::render_dialog_layer(window, cx).is_none()));
            cx.update(|window, _| window.remove_window());
        }
    }

    #[gpui::test]
    fn modal_theme_surfaces_and_scoped_controls_match_original(cx: &mut TestAppContext) {
        cx.set_assets(crate::icons::Assets);
        use gpui_component::button::ButtonVariants;
        use gpui_component::{Root, WindowExt, input::InputState};
        struct Body {
            input: Entity<InputState>,
            textarea: Entity<InputState>,
        }
        impl Render for Body {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        super::button("modal-inside", cx)
                            .primary()
                            .debug_selector(|| "modal-inside".into())
                            .label("Inside")
                            .border_2()
                            .on_click(|_, _, _| {}),
                    )
                    .child(
                        div()
                            .debug_selector(|| "modal-input".into())
                            .child(super::input(&self.input, cx).border_2()),
                    )
                    .child(
                        div()
                            .debug_selector(|| "modal-textarea".into())
                            .child(super::input(&self.textarea, cx).border_1()),
                    )
            }
        }
        struct Probe;
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .size_full()
                    .child(
                        super::button("modal-outside", cx)
                            .debug_selector(|| "modal-outside".into())
                            .label("Outside"),
                    )
                    .children(Root::render_dialog_layer(window, cx))
            }
        }
        cx.update(gpui_component::init);
        cx.update(super::install_dialog_styles);
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/modal-styles-reference.json")).unwrap();
        for case in cases {
            let font = case["rem"].as_f64().unwrap() as f32;
            let custom = case["theme"].as_str().unwrap().contains("custom");
            cx.update(|cx| {
                gpui_component::Theme::global_mut(cx).font_size = px(font);
                let mut styles = NativeStyles::new(Stylesheet::parse([case["css"].as_str().unwrap(), ".page button { background:red !important } #modal-outside {background:#abcdef; border:1px solid black;border-radius:2px}"]).unwrap(), appearance::Palette::default());
                styles.context.push(Target::new("main", "", &["page"]));
                cx.set_global(styles);
            });
            let (_, cx) = cx.add_window_view(|window, cx| {
                window.set_rem_size(px(font));
                let probe = cx.new(|_| Probe);
                Root::new(probe, window, cx)
            });
            cx.update(|window, cx| {
                let body = cx.new(|cx| Body {
                    input: cx.new(|cx| InputState::new(window, cx)),
                    textarea: cx.new(|cx| InputState::new(window, cx).multi_line(true)),
                });
                window.open_dialog(cx, move |dialog, _, _| {
                    dialog
                        .title("Modal title")
                        .child(body.clone())
                        .width(px(500.))
                });
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(500));
            cx.refresh().unwrap();
            cx.run_until_parked();
            let expected_color = |part: &str, field: &str| {
                super::color(
                    case[part][field].as_str().unwrap(),
                    &appearance::Palette::default(),
                )
                .unwrap()
            };
            let paint = cx.update(|window, _| window.painted_quad_opacities_for_test());
            for part in ["content", "header"]
                .into_iter()
                .chain(custom.then_some("body"))
            {
                let bounds = cx.debug_bounds(&format!("native-modal-{part}-0")).unwrap();
                let expected = expected_color(part, "background");
                assert!(
                    paint.iter().any(|(b, color)| *b == bounds
                        && (color.a - expected.a).abs() < 0.001
                        && (color.to_rgb().r - expected.to_rgb().r).abs() < 0.001
                        && (color.to_rgb().g - expected.to_rgb().g).abs() < 0.001
                        && (color.to_rgb().b - expected.to_rgb().b).abs() < 0.001),
                    "{} {part}: {expected:?} at {bounds:?}; {paint:?}",
                    case["theme"]
                );
            }
            let blur = cx.update(|window, _| window.painted_backdrops_for_test());
            for part in ["overlay", "content"] {
                let bounds = cx.debug_bounds(&format!("native-modal-{part}-0")).unwrap();
                let sigma: f32 = case[part]["blur"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("blur(")
                    .trim_end_matches("px)")
                    .parse()
                    .unwrap();
                let radius = case[part]["radius"].as_f64().unwrap() as f32;
                assert!(
                    blur.iter().any(|(b, corners, _, actual, _)| *b == bounds
                        && (f32::from(*actual) - sigma).abs() < 0.001
                        && corners.top_left == px(radius)),
                    "{} {part}: {blur:?}",
                    case["theme"]
                );
            }
            let borders = cx.update(|window, _| window.painted_boxes_for_test());
            for (part, selector) in [
                ("button", "modal-inside"),
                ("input", "modal-input"),
                ("textarea", "modal-textarea"),
            ] {
                let bounds = cx.debug_bounds(selector).unwrap();
                let radius = case[part]["radius"].as_f64().unwrap() as f32;
                assert!(
                    borders
                        .iter()
                        .any(|(b, _, corners, _)| *b == bounds && corners.top_left == px(radius)),
                    "{} {part}: {borders:?}",
                    case["theme"]
                );
            }
            let outside = cx.debug_bounds("modal-outside").unwrap();
            assert!(
                borders
                    .iter()
                    .any(|(b, _, corners, _)| *b == outside && corners.top_left == px(2.))
            );
            // The page selector must still apply outside the portal, and never inside it.
            assert!(
                paint
                    .iter()
                    .any(|(b, color)| *b == outside && *color == rgb(0xff0000).into())
            );
            if custom {
                let button = cx.debug_bounds("modal-inside").unwrap();
                assert!(
                    paint.iter().any(|(b, color)| *b == button
                        && *color == expected_color("button", "background"))
                );
                cx.simulate_mouse_move(button.center(), None, Modifiers::none());
                cx.run_until_parked();
                for state in ["hover", "active"] {
                    if state == "active" {
                        cx.simulate_mouse_down(
                            button.center(),
                            MouseButton::Left,
                            Modifiers::none(),
                        );
                        cx.run_until_parked();
                    }
                    cx.executor()
                        .advance_clock(std::time::Duration::from_millis(200));
                    cx.refresh().unwrap();
                    cx.run_until_parked();
                    let expected = super::color(
                        case[state].as_str().unwrap(),
                        &appearance::Palette::default(),
                    )
                    .unwrap();
                    assert!(
                        cx.update(|window, _| window.painted_quad_opacities_for_test())
                            .iter()
                            .any(|(b, color)| *b == button && *color == expected),
                        "{state}"
                    );
                }
                cx.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::none());
                assert!(
                    cx.update(|window, _| window.painted_shadows_for_test())
                        .iter()
                        .all(|(b, _, _)| *b != cx.debug_bounds("native-modal-content-0").unwrap())
                );
            }
            cx.update(|window, cx| {
                assert_eq!(cx.global::<NativeStyles>().context.len(), 2);
                window.close_all_dialogs(cx);
            });
            cx.run_until_parked();
        }
    }

    #[gpui::test]
    fn generated_surface_styles_reach_slider_and_player_without_leaking(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let mut palette = appearance::Palette::default();
        palette.colors.insert("surface".into(), "#123456".into());
        let sheet = Stylesheet::parse([
            include_str!("../assets/styles/glassy-dark.css"),
            ".mantine-Slider-track { --shade:#123456;color:red;background:blue;opacity:.8 } .mantine-Slider-track::before {background:var(--shade)} .fs-full-screen-player-queue-module-grid-container::before {background:#654321;border:1px solid white}",
        ]).unwrap();
        let styles = NativeStyles::new(sheet, palette);
        let inherited = styles.style_in(
            Target::pseudo("before"),
            [Target::new("div", "", &["mantine-Slider-track"])],
            16.,
        );
        assert_eq!(inherited.text.unwrap().color, Some(rgb(0xff0000).into()));
        assert_eq!(inherited.opacity, None);
        cx.update(|cx| cx.set_global(styles));
        struct Probe {
            slider: Entity<gpui_component::slider::SliderState>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .v_flex()
                    .w(px(500.))
                    .h(px(400.))
                    .child(super::slider(&self.slider, cx).settings(1., "theme-slider"))
                    .child(
                        div()
                            .w_full()
                            .h(px(300.))
                            .child(crate::player_layout::PlayerLayout {
                                open: true,
                                portrait: false,
                                font: px(16.),
                                zoom: 1.,
                                image: Empty.into_any_element(),
                                panel: Empty.into_any_element(),
                            }),
                    )
            }
        }
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| Probe {
                slider: cx.new(|_| {
                    gpui_component::slider::SliderState::new()
                        .min(0.)
                        .max(100.)
                        .default_value(50.)
                }),
            });
            gpui_component::Root::new(view, window, cx)
        });
        cx.run_until_parked();
        let paint = cx.update(|window, _| window.painted_quad_opacities_for_test());
        for (selector, color) in [
            ("theme-slider-track-background", 0x123456),
            ("expanded-player-panel-background", 0x654321),
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            let expected = Hsla::from(rgb(color));
            assert!(
                paint
                    .iter()
                    .any(|(actual, color)| *actual == bounds && *color == expected),
                "{selector}: missing {expected:?} at {bounds:?}; {paint:?}"
            );
        }
        let background = cx.debug_bounds("expanded-player-panel-background").unwrap();
        cx.update(|window, _| {
            assert!(window.painted_borders_for_test().iter().any(
                |(bounds, _, corners, _)| *bounds == background && corners.top_left == px(18.)
            ));
        });
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn normal_line_heights_and_resets_reach_original_rendered_boxes(cx: &mut TestAppContext) {
        let system = cx.use_platform_text_system();
        system
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        struct Probe {
            style: StyleRefinement,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().block().w(px(800.)).child(
                    div()
                        .block()
                        .debug_selector(|| "normal-line-height-box".into())
                        .refine_style(&self.style)
                        .child("Hg"),
                )
            }
        }
        let (view, window_cx) = cx.add_window_view(|_, _| Probe {
            style: StyleRefinement::default(),
        });
        #[cfg(target_os = "linux")]
        let source = include_str!("../assets/css-normal-line-heights-linux-reference.json");
        #[cfg(not(target_os = "linux"))]
        let source = include_str!("../assets/css-normal-line-heights-darwin-reference.json");
        let fixture: serde_json::Value = serde_json::from_str(source).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let size = case["size"].as_f64().unwrap() as f32;
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let css = format!(
                ".parent{{font-family:{};font-size:{size}px;font-weight:{};line-height:2}}.child{{font-size:{size}px;line-height:3;line-height:{}}}",
                case["family"].as_str().unwrap(),
                case["weight"],
                case["reset"].as_str().unwrap()
            );
            let styles = NativeStyles::new(
                Stylesheet::parse([css.as_str()]).unwrap(),
                appearance::Palette::default(),
            )
            .with_font_metrics(system.clone(), gpui::font("Inter"), 2.)
            .with_zoom(zoom);
            let style = styles.style_in(
                Target::new("div", "", &["child"]),
                [Target::new("div", "", &["parent"])],
                16. * zoom,
            );
            let expected = case["height"].as_f64().unwrap() as f32 * zoom;
            let text = style.text.as_ref().unwrap();
            let font = text.font_size.unwrap().to_pixels(px(16. * zoom));
            let actual = f32::from(
                text.line_height
                    .unwrap()
                    .to_pixels(font.into(), px(16. * zoom)),
            );
            assert!(
                (actual - expected).abs() < 0.01,
                "{case}: {actual}, expected {expected}"
            );
            window_cx.update(|_, cx| {
                view.update(cx, |view, cx| {
                    view.style = style;
                    cx.notify();
                })
            });
            window_cx.run_until_parked();
            let bounds = window_cx.debug_bounds("normal-line-height-box").unwrap();
            assert!(
                (f32::from(bounds.size.height) - expected).abs() <= 1.,
                "{case}: {bounds:?}, expected {expected}"
            );
        }
    }

    #[gpui::test]
    fn percentage_tracking_survives_widget_font_changes_and_input_reflow(cx: &mut TestAppContext) {
        use gpui_component::{
            Root,
            input::{InputState, Position},
        };
        use std::{cell::RefCell, rc::Rc};
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        struct Probe {
            style: StyleRefinement,
            sizes: [Pixels; 3],
            input: [Entity<InputState>; 2],
            measured: Rc<RefCell<[f32; 3]>>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                let fraction = self
                    .style
                    .text
                    .as_ref()
                    .unwrap()
                    .letter_spacing_fraction
                    .unwrap();
                div()
                    .v_flex()
                    .font_family("Inter")
                    .refine_style(&self.style)
                    .children(self.sizes.into_iter().enumerate().map(|(index, font)| {
                        let measured = self.measured.clone();
                        div().text_size(font).child(
                            canvas(
                                move |_, window, _| {
                                    let style = window.text_style();
                                    let size = style.font_size.to_pixels(window.rem_size());
                                    let mut run = style.to_run(1);
                                    let actual = window
                                        .text_system()
                                        .layout_line("H", size, &[run.clone()], None)
                                        .width;
                                    run.letter_spacing = px(0.);
                                    run.letter_spacing_fraction = 0.;
                                    let plain = window
                                        .text_system()
                                        .layout_line("H", size, &[run], None)
                                        .width;
                                    measured.borrow_mut()[index] = f32::from(actual - plain);
                                },
                                |_, _, _, _| {},
                            )
                            .w(px(1.))
                            .h(px(1.)),
                        )
                    }))
                    .children(self.input.iter().enumerate().map(|(index, input)| {
                        div()
                            .debug_selector(move || format!("percentage-input-{index}"))
                            .w(px(90.))
                            .h(px(240.))
                            .child(
                                Input::new(input)
                                    .bordered(false)
                                    .p_0()
                                    .h(px(240.))
                                    .text_size(self.sizes[2])
                                    .line_height(relative(1.5))
                                    .when(index == 1, |input| {
                                        input.letter_spacing(self.sizes[2] * fraction)
                                    }),
                            )
                    }))
            }
        }
        let captured = Rc::new(RefCell::new(None));
        let measured = Rc::new(RefCell::new([0.; 3]));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let input = std::array::from_fn(|_| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .multi_line(true)
                        .rows(8)
                        .default_value("HHHHHHHHHHHHHHHHHHHH")
                })
            });
            let probe = cx.new(|_| Probe {
                style: NativeStyles::new(
                    Stylesheet::parse([".parent{letter-spacing:10%}"]).unwrap(),
                    Default::default(),
                )
                .style(Target::new("div", "", &["parent"]), None, 16.),
                sizes: [px(10.), px(20.), px(30.)],
                input,
                measured: measured.clone(),
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        let inputs = probe.read_with(cx, |view, _| view.input.clone());
        #[cfg(target_os = "linux")]
        let source = include_str!("../assets/css-relative-fonts-linux-reference.json");
        #[cfg(not(target_os = "linux"))]
        let source = include_str!("../assets/css-relative-fonts-darwin-reference.json");
        let reference: serde_json::Value = serde_json::from_str(source).unwrap();
        for case in reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| {
                matches!(
                    case["name"].as_str(),
                    Some(
                        "spacing-percentage"
                            | "spacing-negative-percentage"
                            | "spacing-percentage-inherit"
                    )
                )
            })
        {
            for zoom in [1., 1.5] {
                let root = case["rootFont"].as_f64().unwrap() as f32;
                let native = NativeStyles::new(
                    Stylesheet::parse([case["css"].as_str().unwrap()]).unwrap(),
                    Default::default(),
                )
                .with_zoom(zoom);
                let style = native.style(Target::new("div", "", &["parent"]), None, root * zoom);
                let sizes = std::array::from_fn(|index| {
                    px(case["elements"][index]["fontSize"].as_f64().unwrap() as f32 * zoom)
                });
                probe.update(cx, |view, cx| {
                    view.style = style;
                    view.sizes = sizes;
                    cx.notify();
                });
                cx.run_until_parked();
                for (index, actual) in measured.borrow().iter().enumerate() {
                    let expected =
                        case["elements"][index]["letterSpacing"].as_f64().unwrap() as f32 * zoom;
                    assert!(
                        (actual - expected).abs() < zoom / 64. + 0.002,
                        "{} child {index} zoom {zoom}: {actual} != {expected}",
                        case["name"]
                    );
                }
                let mut carets = Vec::new();
                for (index, input) in inputs.iter().enumerate() {
                    cx.update(|window, cx| {
                        window.activate_window();
                        window.focus(&input.read(cx).focus_handle(cx));
                        input.update(cx, |input, cx| {
                            input.set_cursor_position(Position::new(0, 20), window, cx)
                        });
                    });
                    cx.run_until_parked();
                    let origin = cx
                        .debug_bounds(&format!("percentage-input-{index}"))
                        .unwrap()
                        .origin;
                    let position = cx.update(|window, cx| {
                        window
                            .painted_lines_and_quads_for_test()
                            .into_iter()
                            .find(|(bounds, color, _, _)| {
                                *color == cx.theme().caret
                                    && bounds.size.width
                                        <= ScaledPixels::from(2. * window.scale_factor())
                            })
                            .map(|(bounds, _, _, _)| {
                                point(
                                    px(f64::from(bounds.origin.x) as f32 / window.scale_factor()),
                                    px(f64::from(bounds.origin.y) as f32 / window.scale_factor()),
                                )
                            })
                            .expect("Focused percentage-tracked input caret")
                    });
                    carets.push(position - origin);
                }
                assert!(
                    (carets[0] - carets[1]).magnitude() <= 1.,
                    "{} zoom {zoom}: inherited and explicit input carets {carets:?}",
                    case["name"]
                );
            }
        }
    }

    #[test]
    fn relative_fonts_lengths_and_inheritance_match_original_css() {
        #[cfg(target_os = "linux")]
        let source = include_str!("../assets/css-relative-fonts-linux-reference.json");
        #[cfg(not(target_os = "linux"))]
        let source = include_str!("../assets/css-relative-fonts-darwin-reference.json");
        let fixture: serde_json::Value = serde_json::from_str(source).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let root = case["rootFont"].as_f64().unwrap() as f32;
            let base = format!("body {{ font-size:{root}px; line-height:1.25; font-weight:400; }}");
            for zoom in [1., 1.5] {
                let native = NativeStyles::new(
                    Stylesheet::parse([base.as_str(), case["css"].as_str().unwrap()]).unwrap(),
                    appearance::Palette::default(),
                )
                .with_zoom(zoom);
                let mut parents = Vec::new();
                for expected in case["elements"].as_array().unwrap() {
                    let class = expected["name"].as_str().unwrap();
                    let target = Target::new("div", "", &[class]);
                    let style = native.style_in(target.clone(), parents.clone(), root * zoom);
                    parents.push(target);
                    let text = style.text.clone().unwrap_or_default();
                    let font = text.font_size.unwrap().to_pixels(px(root * zoom));
                    let close = |actual: f32, property: &str| {
                        let expected_value = expected[property].as_f64().unwrap() as f32 * zoom;
                        // Percentage spacing is measured from Chromium's quantized inline width.
                        let tolerance = if property == "letterSpacing"
                            && case["name"].as_str().unwrap().starts_with("spacing-")
                            && case["name"].as_str().unwrap().contains("percentage")
                        {
                            zoom / 64. + 0.002
                        } else {
                            0.002
                        };
                        assert!(
                            (actual - expected_value).abs() < tolerance,
                            "{} {class} at zoom {zoom}: {property} {actual} vs {expected_value}, style {style:?}",
                            case["name"]
                        );
                    };
                    close(f32::from(font), "fontSize");
                    close(
                        f32::from(
                            text.line_height
                                .unwrap()
                                .to_pixels(font.into(), px(root * zoom)),
                        ),
                        "lineHeight",
                    );
                    assert_eq!(
                        text.font_weight.unwrap().0,
                        expected["fontWeight"].as_f64().unwrap() as f32,
                        "{case}, {class}"
                    );
                    close(
                        f32::from(
                            text.letter_spacing.unwrap_or(px(0.))
                                + font * text.letter_spacing_fraction.unwrap_or(0.),
                        ),
                        "letterSpacing",
                    );
                    close(
                        f32::from(
                            style
                                .padding
                                .top
                                .unwrap_or_default()
                                .to_pixels(font.into(), px(root * zoom)),
                        ),
                        "paddingTop",
                    );
                    close(
                        f32::from(
                            style
                                .border_widths
                                .top
                                .unwrap_or_default()
                                .to_pixels(px(root * zoom)),
                        ),
                        "borderTop",
                    );
                    close(
                        f32::from(
                            style
                                .corner_radii
                                .top_left
                                .unwrap_or_default()
                                .to_pixels(px(root * zoom)),
                        ),
                        "radius",
                    );
                    let expected_shadows = feishin_core::styles::text_shadows(
                        expected["shadow"].as_str().unwrap(),
                        root,
                    )
                    .unwrap();
                    let actual_shadows = text.text_shadows.unwrap_or_default();
                    assert_eq!(actual_shadows.len(), expected_shadows.len());
                    for (actual, expected) in actual_shadows.iter().zip(expected_shadows) {
                        assert!(
                            (f32::from(actual.offset.x) - expected.offset[0] * zoom).abs() < 0.002
                                && (f32::from(actual.offset.y) - expected.offset[1] * zoom).abs()
                                    < 0.002
                                && (f32::from(actual.blur_radius) - expected.blur_radius * zoom)
                                    .abs()
                                    < 0.002,
                            "{case}, {class}, {actual:?}"
                        );
                    }
                }
            }
        }
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn relative_font_styles_reach_rendered_text_boxes(cx: &mut TestAppContext) {
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        struct Probe {
            styles: Vec<StyleRefinement>,
            zoom: f32,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .block()
                    .font_family("Inter")
                    .children(self.styles.iter().enumerate().map(|(index, style)| {
                        div()
                            .block()
                            .w(px(400. * self.zoom))
                            .debug_selector(move || format!("relative-font-{index}"))
                            .refine_style(style)
                            .child("Hg")
                    }))
            }
        }
        #[cfg(target_os = "linux")]
        let source = include_str!("../assets/css-relative-fonts-linux-reference.json");
        #[cfg(not(target_os = "linux"))]
        let source = include_str!("../assets/css-relative-fonts-darwin-reference.json");
        let fixture: serde_json::Value = serde_json::from_str(source).unwrap();
        let (view, window_cx) = cx.add_window_view(|_, _| Probe {
            styles: vec![],
            zoom: 1.,
        });
        for case in fixture["cases"].as_array().unwrap() {
            let root = case["rootFont"].as_f64().unwrap() as f32;
            let base = format!("body {{ font-size:{root}px; line-height:1.25; font-weight:400; }}");
            for zoom in [1., 1.5] {
                let native = NativeStyles::new(
                    Stylesheet::parse([base.as_str(), case["css"].as_str().unwrap()]).unwrap(),
                    appearance::Palette::default(),
                )
                .with_zoom(zoom);
                let mut parents = Vec::new();
                let styles = case["elements"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|expected| {
                        let target = Target::new("div", "", &[expected["name"].as_str().unwrap()]);
                        let style = native.style_in(target.clone(), parents.clone(), root * zoom);
                        parents.push(target);
                        style
                    })
                    .collect();
                window_cx.update(|_, cx| {
                    view.update(cx, |view, cx| {
                        view.styles = styles;
                        view.zoom = zoom;
                        cx.notify();
                    })
                });
                window_cx.run_until_parked();
                for (index, expected) in case["elements"].as_array().unwrap().iter().enumerate() {
                    let bounds = window_cx.debug_bounds(&format!("relative-font-{index}"));
                    let height = (expected["lineHeight"].as_f64().unwrap()
                        + 2. * expected["paddingTop"].as_f64().unwrap()
                        + 2. * expected["borderTop"].as_f64().unwrap())
                        as f32
                        * zoom;
                    assert!(
                        (f32::from(bounds.unwrap().size.height) - height).abs() <= 1.,
                        "{} node {index}, zoom {zoom}: {bounds:?}, expected height {height}",
                        case["name"]
                    );
                }
            }
        }
    }

    #[test]
    fn css_text_shadow_cascade_matches_original_renderer() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/css-text-shadow-darwin-reference.json"
        ))
        .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let styles = super::NativeStyles::new(
                feishin_core::styles::Stylesheet::parse([case["css"].as_str().unwrap()]).unwrap(),
                feishin_core::appearance::Palette::default(),
            );
            let parent = feishin_core::styles::Target::new("div", "", &["parent"]);
            let parent_style = styles.style(parent.clone(), None, 16.);
            let style = styles.style_in(
                feishin_core::styles::Target::new("div", "", &["text"]),
                [parent],
                16.,
            );
            let color = style
                .text
                .as_ref()
                .and_then(|text| text.color)
                .or_else(|| parent_style.text.as_ref().and_then(|text| text.color))
                .unwrap_or_else(gpui::white);
            let actual = style
                .text
                .and_then(|text| text.text_shadows)
                .unwrap_or_default();
            let expected = feishin_core::styles::text_shadows(
                case["computed"]["shadow"].as_str().unwrap(),
                16.,
            )
            .unwrap();
            assert_eq!(actual.len(), expected.len(), "{case}");
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert_eq!(
                    [f32::from(actual.offset.x), f32::from(actual.offset.y)],
                    expected.offset,
                    "{case}"
                );
                assert_eq!(
                    f32::from(actual.blur_radius),
                    expected.blur_radius,
                    "{case}"
                );
                let actual: gpui::Rgba = actual.color.unwrap_or(color).into();
                for (actual, expected) in [actual.r, actual.g, actual.b, actual.a]
                    .into_iter()
                    .zip(expected.color.unwrap())
                {
                    assert!((actual - expected).abs() < 0.002, "{case}");
                }
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn css_text_shadows_match_original_glyph_layers(cx: &mut gpui::TestAppContext) {
        use gpui::{Context, IntoElement, Render, prelude::*, px};
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        struct Probe(serde_json::Value, super::NativeStyles, bool);
        impl Render for Probe {
            fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
                let parent = feishin_core::styles::Target::new("div", "", &["parent"]);
                let parent_style = self.1.style(parent.clone(), None, 16.);
                let child_style = self.1.style_in(
                    feishin_core::styles::Target::new("div", "", &["text"]),
                    [parent],
                    16.,
                );
                gpui::div()
                    .relative()
                    .w(px(256.))
                    .h(px(160.))
                    .bg(gpui::rgb(0x777777))
                    .font_family("Inter")
                    .font_features(gpui::FontFeatures(std::sync::Arc::new(vec![(
                        "tnum".into(),
                        1,
                    )])))
                    .text_color(gpui::white())
                    .child(
                        gpui::div()
                            .absolute()
                            .left(px(32.))
                            .top(px(32.))
                            .w(px(160.))
                            .refine_style(&parent_style)
                            .child(
                                gpui::div()
                                    .block()
                                    .w_full()
                                    .text_size(px(16.))
                                    .font_weight(gpui::FontWeight(700.))
                                    .line_height(gpui::relative(1.1))
                                    .when(self.0["wrap"] != true, |div| div.whitespace_nowrap())
                                    .refine_style(&child_style)
                                    .when(!self.2, |div| div.text_shadows(vec![]))
                                    .child(if self.0["name"] == "standalone" {
                                        let text = self.0["text"].as_str().unwrap().to_owned();
                                        gpui::canvas(
                                            |_, _, _| {},
                                            move |bounds, _, window, cx| {
                                                let run = window.text_style().to_run(text.len());
                                                let line = window.text_system().shape_line(
                                                    text.clone().into(),
                                                    px(16.),
                                                    &[run],
                                                    None,
                                                );
                                                line.paint(bounds.origin, px(17.6), window, cx)
                                                    .unwrap();
                                            },
                                        )
                                        .w_full()
                                        .h(px(17.6))
                                        .into_any_element()
                                    } else {
                                        gpui::StyledText::new(
                                            self.0["text"].as_str().unwrap().to_owned(),
                                        )
                                        .into_any_element()
                                    }),
                            ),
                    )
            }
        }
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/css-text-shadow-linux-reference.json")
        } else {
            include_str!("../assets/css-text-shadow-darwin-reference.json")
        })
        .unwrap();
        let mut differences = Vec::new();
        for case in reference["cases"].as_array().unwrap() {
            let mut native = Vec::new();
            let mut original = Vec::new();
            for (shadow, key) in [(false, "plain"), (true, "shadow")] {
                let styles = super::NativeStyles::new(
                    feishin_core::styles::Stylesheet::parse([case["css"].as_str().unwrap()])
                        .unwrap(),
                    feishin_core::appearance::Palette::default(),
                );
                let (_, window_cx) = cx.add_window_view(|_, _| Probe(case.clone(), styles, shadow));
                window_cx.run_until_parked();
                native.push(window_cx.update(|window, _| {
                    window
                        .rasterized_scene_for_test(gpui::size(px(256.), px(160.)))
                        .unwrap()
                }));
                let filename = case[key].as_str().unwrap();
                let image = image::open(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets")
                        .join(filename),
                )
                .unwrap()
                .into_rgba8();
                assert_eq!(image.dimensions(), (512, 320));
                original.push(image.into_raw());
                if let Some(directory) = std::env::var_os("FEISHIN_CSS_TEXT_SHADOW_ACTUAL") {
                    std::fs::create_dir_all(&directory).unwrap();
                    image::save_buffer(
                        std::path::Path::new(&directory).join(filename),
                        native.last().unwrap(),
                        512,
                        320,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
            if case["name"] == "transparent-ink" || case["name"] == "transparent-emoji" {
                assert!(original[0].chunks_exact(4).all(|p| p[..3] == [119; 3]));
                assert!(
                    native[0].chunks_exact(4).all(|p| p[..3] == [119; 3]),
                    "Transparent glyphs remain visible: {case}"
                );
            }
            if case["background"] != true && case["border"] != true {
                let ink_bounds = |pixels: &[u8]| {
                    let mut bounds = [u32::MAX, u32::MAX, 0, 0];
                    for (index, p) in pixels.chunks_exact(4).enumerate() {
                        if p[..3].iter().all(|n| n.abs_diff(119) <= 2) {
                            continue;
                        }
                        let x = index as u32 % 512;
                        let y = index as u32 / 512;
                        bounds[0] = bounds[0].min(x);
                        bounds[1] = bounds[1].min(y);
                        bounds[2] = bounds[2].max(x);
                        bounds[3] = bounds[3].max(y);
                    }
                    bounds
                };
                for (actual, expected) in ink_bounds(&native[0])
                    .into_iter()
                    .zip(ink_bounds(&original[0]))
                {
                    assert!(
                        actual.abs_diff(expected) <= 2,
                        "Glyph bounds differ: {case}, {actual}, {expected}"
                    );
                }
            }
            let mut squared = 0_f64;
            let mut maximum = 0_i16;
            let mut samples = 0_usize;
            for index in (0..native[0].len()).step_by(4) {
                // Isolate shadows from foreground font antialiasing. Colored backgrounds
                // stay in this comparison, exposing accidental background/border shadows.
                let plain = &original[0][index..index + 3];
                if plain != &native[0][index..index + 3]
                    || ![&[119, 119, 119][..], &[68, 85, 102][..]].contains(&plain)
                {
                    continue;
                }
                for channel in 0..3 {
                    let expected = i16::from(original[0][index + channel])
                        - i16::from(original[1][index + channel]);
                    let actual = i16::from(native[0][index + channel])
                        - i16::from(native[1][index + channel]);
                    let error = (actual - expected).abs();
                    squared += f64::from(error).powi(2);
                    let sharp_edge = if case["name"] == "sharp" && error > 12 {
                        let x = index as i32 / 4 % 512 - 12;
                        let y = index as i32 / 4 / 512 - 10;
                        let near_edge = |pixels: &[u8]| {
                            if !(1..511).contains(&x) || !(1..319).contains(&y) {
                                return false;
                            }
                            let shade = |x: i32, y: i32| pixels[((y * 512 + x) * 4) as usize];
                            let center = shade(x, y);
                            (center > 119 && center < 253)
                                || (-1..=1).any(|dy| {
                                    (-1..=1)
                                        .any(|dx| (shade(x + dx, y + dy) > 119) != (center > 119))
                                })
                        };
                        let edge = near_edge(&original[0]) || near_edge(&native[0]);
                        assert!(
                            error <= if cfg!(target_os = "linux") { 112 } else { 40 } && edge,
                            "Sharp shadow differs beyond glyph antialiasing: {case}, {x},{y}: {error}"
                        );
                        edge
                    } else {
                        false
                    };
                    if !sharp_edge {
                        maximum = maximum.max(error);
                    }
                    samples += 1;
                }
            }
            assert!(samples > 100_000, "Missing background samples: {case}");
            let rms = (squared / samples as f64).sqrt();
            let maximum_limit = if cfg!(target_os = "linux") {
                22
            } else if case["name"] == "wrap" {
                20
            } else {
                12
            };
            let rms_limit = if cfg!(target_os = "linux") {
                if case["name"] == "sharp" { 2.6 } else { 1.1 }
            } else {
                0.7
            };
            if rms > rms_limit || maximum > maximum_limit {
                differences.push(format!("RMS {rms}, maximum {maximum}: {}", case["name"]));
            }
        }
        assert!(
            differences.is_empty(),
            "CSS text shadow differences: {differences:#?}"
        );
    }
    use super::*;
    use ::core::prelude::v1::test;

    #[test]
    fn css_shorthands_follow_cascade_order_and_preview_gradients_reach_native_style() {
        let palette = appearance::Palette::default();
        let sheet = Stylesheet::parse([
            "#probe { padding-left: 17px; border-top: 5px solid #123456; }
             div { padding: 2px; border: none; }
             #probe { margin-left: 11px; margin: 3px; inset: 1px 2px 3px 4px;
                      background: linear-gradient(135deg, rgb(255 255 255 / 10%) 0%, rgb(0 0 0 / 10%) 100%); }
             .preview { padding-top: 9px !important; }
             #probe { padding: 4px; }",
        ]).unwrap();
        let native = NativeStyles::new(sheet, palette.clone());
        let style = native.style(Target::new("div", "probe", &["preview"]), None, 16.);
        assert_eq!(style.padding.left, Some(px(4.).into()));
        assert_eq!(style.padding.top, Some(px(9.).into()));
        assert_eq!(style.margin.left, Some(px(3.).into()));
        assert_eq!(style.border_widths.top, Some(px(5.).into()));
        assert_eq!(style.border_widths.left, Some(px(0.).into()));
        assert_eq!(style.border_color, color("#123456", &palette));
        assert_eq!(style.inset.left, Some(px(4.).into()));
        assert_eq!(style.inset.bottom, Some(px(3.).into()));
        assert!(matches!(style.background, Some(Fill::Image(_))));
        let sheet =
            Stylesheet::parse(["#probe { padding-left: 17px; } div { padding: 2px; }"]).unwrap();
        let native = NativeStyles::new(sheet, palette);
        let style = native.style(Target::new("div", "probe", &[]), None, 16.);
        assert_eq!(style.padding.left, Some(px(17.).into()));
        assert_eq!(style.padding.top, Some(px(2.).into()));
    }

    #[gpui::test]
    fn css_gradient_backgrounds_paint_resize_and_preserve_longhand_colors(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        struct Probe;
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                super::apply(div().w(px(97.)).h(px(37.)), "div", "probe", &[], None, cx)
            }
        }
        let native = NativeStyles::new(Stylesheet::parse([
            "#probe { background-color: #123; background-image: linear-gradient(to top right, transparent 4px, #fa4 30%, blue 33px); border-radius: 7px; opacity: .6; }"
        ]).unwrap(), appearance::Palette::default());
        let style = native.style(Target::new("div", "probe", &[]), None, 16.);
        let Some(Fill::Image(image)) = style.background else {
            panic!("Gradient was discarded")
        };
        assert_eq!(
            image.color,
            color("#123", &appearance::Palette::default()).map(Into::into)
        );
        cx.update(|cx| cx.set_global(native));
        let (_, cx) = cx.add_window_view(|_, _| Probe);
        cx.run_until_parked();
        cx.update(|window, _| {
            let painted = window.painted_images_for_test();
            assert_eq!(painted.len(), 1);
            assert_eq!(painted[0].0.size, size(px(97.), px(37.)));
            assert_eq!(painted[0].1.top_left, px(7.));
            assert_eq!(painted[0].2, 0.6);
            let a = image.image(size(px(97.), px(37.)), window);
            let b = image.image(size(px(97.), px(37.)), window);
            assert!(std::sync::Arc::ptr_eq(&a, &b));
            assert_eq!(a.size(0), size(DevicePixels(194), DevicePixels(74)));
            let changed = image.image(size(px(64.), px(24.)), window);
            assert!(!std::sync::Arc::ptr_eq(&a, &changed));
            assert!(std::sync::Arc::ptr_eq(
                &a,
                &image.image(size(px(97.), px(37.)), window)
            ));
            for width in 100..110 {
                image.image(size(px(width as f32), px(24.)), window);
            }
            assert!(!std::sync::Arc::ptr_eq(
                &a,
                &image.image(size(px(97.), px(37.)), window)
            ));
        });
        for (css, expected) in [
            (
                "background-image: linear-gradient(red, green, blue); background-color: #123",
                "#123",
            ),
            (
                "background: linear-gradient(red, green, blue); background-image: none; background-color: #123",
                "#123",
            ),
            (
                "background-color: #123; background-image: linear-gradient(red, green, blue); background-image: none",
                "#123",
            ),
            (
                "background-color: #123; background: linear-gradient(red, green, blue)",
                "transparent",
            ),
            (
                "background-color: #123; background-color: linear-gradient(red, green, blue)",
                "#123",
            ),
            (
                "background: linear-gradient(red, green, blue); background: none",
                "transparent",
            ),
        ] {
            let palette = appearance::Palette::default();
            let native = NativeStyles::new(
                Stylesheet::parse([format!("#probe {{ {css} }}").as_str()]).unwrap(),
                palette.clone(),
            );
            let style = native.style(Target::new("div", "probe", &[]), None, 16.);
            let actual = style
                .background
                .as_ref()
                .and_then(Fill::color)
                .unwrap_or_else(|| transparent_black().into());
            assert_eq!(actual, color(expected, &palette).unwrap().into(), "{css}");
        }
        let palette = appearance::resolve("defaultDark", std::path::Path::new("/missing")).unwrap();
        let native = NativeStyles::new(
            Stylesheet::parse(["#probe { background: var(--theme-overlay-header); }"]).unwrap(),
            palette,
        );
        assert!(matches!(
            native
                .style(Target::new("div", "probe", &[]), None, 16.)
                .background,
            Some(Fill::Image(_))
        ));
        for (property, preserve) in [("background-image", true), ("background", false)] {
            let palette = appearance::Palette::default();
            let native = NativeStyles::new(
                Stylesheet::parse([format!(
                    "#probe {{ {property}: linear-gradient(transparent, red, blue) }}"
                )
                .as_str()])
                .unwrap(),
                palette.clone(),
            );
            let refinement = native.style(Target::new("div", "probe", &[]), None, 16.);
            let original = color("#fed", &palette).unwrap();
            let mut element = div().bg(original).refine_style(&refinement);
            assert_eq!(
                element.style().background.as_ref().and_then(Fill::color),
                preserve.then(|| original.into())
            );
        }
    }

    #[gpui::test]
    fn multiline_placeholders_wrap_and_keep_empty_carets_at_the_paragraph_edge(
        cx: &mut TestAppContext,
    ) {
        use gpui_component::{
            Root,
            input::{InputState, Position},
        };
        let placeholder =
            "antidisestablishmentarianismantidisestablishmentarianism\nABC אבג DEF אבג ABC 123";
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        struct Probe {
            input: Entity<InputState>,
            width: f32,
            direction: TextDirection,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .w(px(self.width + 10.))
                    .h(px(300.))
                    .debug_selector(|| "placeholder-field".into())
                    .text_direction(self.direction)
                    .text_align(TextAlign::Start)
                    .font_fallbacks(FontFallbacks::from_fonts(vec!["Noto Sans Hebrew".into()]))
                    .child(
                        Input::new(&self.input)
                            .bordered(false)
                            .h(px(300.))
                            .p_0()
                            .font_family("Inter")
                            .text_size(px(14.))
                            .line_height(relative(1.5)),
                    )
            }
        }
        let captured = std::rc::Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .rows(15)
                    .placeholder(placeholder)
            });
            let probe = cx.new(|_| Probe {
                input,
                width: 42.,
                direction: TextDirection::Ltr,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        let input = cx.update(|_, cx| probe.read(cx).input.clone());
        for width in [42., 70.] {
            for direction in [TextDirection::Ltr, TextDirection::Rtl] {
                cx.update(|window, cx| {
                    probe.update(cx, |probe, cx| {
                        probe.width = width;
                        probe.direction = direction;
                        cx.notify();
                    });
                    input.update(cx, |input, cx| {
                        input.set_cursor_position(Position::new(0, 0), window, cx)
                    });
                });
                cx.run_until_parked();
                let field = cx.debug_bounds("placeholder-field").unwrap();
                cx.update(|window, cx| {
                    let candidate = input.update(cx, |input, cx| {
                        EntityInputHandler::bounds_for_range(
                            input,
                            0..0,
                            Bounds::default(),
                            window,
                            cx,
                        )
                        .unwrap()
                    });
                    let expected = field.left()
                        + if direction == TextDirection::Rtl {
                            px(width)
                        } else {
                            px(0.)
                        };
                    assert!(
                        (candidate.left() - expected).abs() < px(0.1),
                        "Empty caret followed placeholder text: {candidate:?}, {expected:?}"
                    );
                    let rows = window
                        .painted_glyphs_for_test()
                        .into_iter()
                        .filter(|(_, _, color)| *color == cx.theme().muted_foreground)
                        .map(|(bounds, _, _)| {
                            ((bounds.center().y - candidate.top()) / px(21.)).floor() as i32
                        })
                        .collect::<std::collections::BTreeSet<_>>();
                    assert!(rows.len() > 3, "Placeholder did not wrap: {rows:?}");
                    assert_eq!(input.read(cx).value(), "");
                    assert_eq!(input.read(cx).cursor_position(), Position::new(0, 0));
                });
            }
        }
    }

    #[gpui::test]
    fn masked_unicode_ime_ranges_and_point_indices_follow_displayed_cells(cx: &mut TestAppContext) {
        use gpui_component::{
            Root,
            input::{InputState, Position},
        };
        const SECRET: &str = "A🙂e\u{301}אבZ";
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        struct Probe(Entity<InputState>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().w(px(210.)).text_direction(TextDirection::Ltr).child(
                    Input::new(&self.0)
                        .bordered(false)
                        .h(px(21.))
                        .p_0()
                        .font_family("Inter")
                        .text_size(px(14.))
                        .line_height(relative(1.5)),
                )
            }
        }
        let captured = std::rc::Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| {
                let mut input = InputState::new(window, cx).masked(true);
                input.set_value(SECRET, window, cx);
                input
            });
            captured.replace(Some(input.clone()));
            let probe = cx.new(|_| Probe(input));
            Root::new(probe, window, cx)
        });
        let input = captured.borrow_mut().take().unwrap();
        let mut carets = vec![];
        let mut utf16_offsets = vec![0];
        for character in SECRET.chars() {
            utf16_offsets.push(utf16_offsets.last().unwrap() + character.len_utf16());
        }
        for (column, offset) in utf16_offsets.iter().enumerate() {
            cx.update(|window, cx| {
                input.update(cx, |input, cx| {
                    input.set_cursor_position(Position::new(0, column as u32), window, cx)
                })
            });
            cx.run_until_parked();
            let caret = cx.update(|window, cx| {
                input.update(cx, |input, cx| {
                    EntityInputHandler::bounds_for_range(
                        input,
                        *offset..*offset,
                        Bounds::default(),
                        window,
                        cx,
                    )
                    .unwrap()
                })
            });
            carets.push(caret);
        }
        let bounds = Bounds::new(carets[0].origin, size(px(200.), px(21.)));
        for (column, range) in utf16_offsets.windows(2).enumerate() {
            cx.update(|window, cx| {
                input.update(cx, |input, cx| {
                    let rect = EntityInputHandler::bounds_for_range(
                        input,
                        range[0]..range[1],
                        bounds,
                        window,
                        cx,
                    )
                    .unwrap();
                    assert!((rect.left() - carets[column].left()).abs() < px(0.1));
                    assert!(
                        (rect.right() - carets[column + 1].left()).abs() < px(0.1),
                        "Masked IME range used source bytes: {range:?}, {rect:?}"
                    );
                    let point = point(
                        carets[column].left()
                            + (carets[column + 1].left() - carets[column].left()) * 0.1,
                        carets[column].center().y,
                    );
                    assert_eq!(
                        EntityInputHandler::character_index_for_point(input, point, window, cx),
                        Some(range[0])
                    );
                    assert_eq!(input.value(), SECRET);
                })
            });
        }
    }

    #[gpui::test]
    fn native_popup_css_reaches_painted_keyboard_hover_checked_and_disabled_items(
        cx: &mut TestAppContext,
    ) {
        popup_css_kind(gpui_component::menu::PopupMenuKind::Context, cx);
    }

    #[gpui::test]
    fn native_dropdown_css_isolated_from_context_selectors_reaches_painted_items(
        cx: &mut TestAppContext,
    ) {
        popup_css_kind(gpui_component::menu::PopupMenuKind::Dropdown, cx);
    }

    fn popup_css_kind(kind: gpui_component::menu::PopupMenuKind, cx: &mut TestAppContext) {
        use gpui_component::{
            Root,
            menu::{DropdownMenu, PopupMenuItem},
        };
        use std::rc::Rc;
        struct Probe {
            colors: Vec<Rc<RefCell<Option<Hsla>>>>,
            kind: gpui_component::menu::PopupMenuKind,
        }
        impl Render for Probe {
            fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let colors = self.colors.clone();
                let kind = self.kind;
                div().p_4().child(
                    super::button("popup-css-probe", cx)
                        .debug_selector(|| "popup-css-probe".into())
                        .label("Open")
                        .dropdown_menu(move |mut menu, _, _| {
                            menu = menu.kind(kind);
                            for (index, color) in colors.iter().enumerate() {
                                let color = color.clone();
                                menu = menu.item(
                                    PopupMenuItem::element(move |_, _| {
                                        let color = color.clone();
                                        div().w(px(100.)).h(px(20.)).child(
                                            canvas(
                                                |_, _, _| (),
                                                move |_, _, window, _| {
                                                    color.replace(Some(window.text_style().color));
                                                },
                                            )
                                            .size(px(10.)),
                                        )
                                    })
                                    .disabled(index == 1)
                                    .checked(index == 2)
                                    .on_click(|_, _, _| {}),
                                );
                            }
                            menu
                        }),
                )
            }
        }
        cx.update(gpui_component::init);
        cx.update(|cx| {
            cx.set_global(NativeStyles::new(
                Stylesheet::parse(["
                .fs-context-menu-module-content .fs-context-menu-module-item, .fs-dropdown-menu-module-menu-dropdown .fs-dropdown-menu-module-menu-item { color: #112233; }
                .fs-context-menu-module-item[data-highlighted], .fs-dropdown-menu-module-menu-item[data-hovered] { color: #223344; }
                .fs-context-menu-module-item[data-disabled=''], .fs-dropdown-menu-module-menu-item:disabled { color: #334455; }
                .fs-context-menu-module-item:checked, .fs-dropdown-menu-module-menu-item:checked { color: #556677; }
                .fs-context-menu-module-item:hover:active, .fs-dropdown-menu-module-menu-item:hover:active { color: #667788; }
                .fs-context-menu-module-content .fs-dropdown-menu-module-menu-item, .fs-dropdown-menu-module-menu-dropdown .fs-context-menu-module-item { color: #ff0000 !important; }
                button.fs-context-menu-module-item, div.fs-dropdown-menu-module-menu-item { color: #ff0000 !important; }
            "])
                .unwrap(),
                appearance::Palette::default(),
            ));
            super::install_popup_styles(cx);
        });
        let colors = (0..3)
            .map(|_| Rc::new(RefCell::new(None)))
            .collect::<Vec<_>>();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let probe = cx.new(|_| Probe {
                colors: colors.clone(),
                kind,
            });
            Root::new(probe, window, cx)
        });
        cx.run_until_parked();
        let button = cx.debug_bounds("popup-css-probe").unwrap().center();
        cx.simulate_click(button, Modifiers::none());
        cx.run_until_parked();
        let accessible =
            cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        assert!(
            accessible
                .nodes
                .iter()
                .any(|(_, node)| node.role() == gpui::Role::Menu)
        );
        let items = accessible
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == gpui::Role::MenuItem)
            .collect::<Vec<_>>();
        assert_eq!(items.len(), 3);
        assert_eq!(
            items.iter().filter(|(_, node)| node.is_disabled()).count(),
            1
        );
        assert_eq!(
            items
                .iter()
                .filter(|(_, node)| node.toggled() == Some(gpui::Toggled::True))
                .count(),
            1
        );
        let _consumer = accesskit_consumer::Tree::new(accessible, true);
        let expected = |hex| super::color(hex, &appearance::Palette::default()).unwrap();
        assert_eq!(*colors[0].borrow(), Some(expected("#112233")));
        assert_eq!(*colors[1].borrow(), Some(expected("#334455")));
        assert_eq!(*colors[2].borrow(), Some(expected("#556677")));
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        assert_eq!(*colors[0].borrow(), Some(expected("#223344")));
        let first = cx.debug_bounds("popup-menu-item-0").unwrap().center();
        cx.simulate_mouse_move(first, None, Modifiers::none());
        cx.simulate_mouse_down(first, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(*colors[0].borrow(), Some(expected("#667788")));
        cx.simulate_mouse_up(first, MouseButton::Left, Modifiers::none());
    }
    #[gpui::test]
    fn native_css_interaction_states_reach_painted_buttons_and_inputs(cx: &mut TestAppContext) {
        use gpui_component::{Disableable, Root, Selectable, input::InputState};
        use std::rc::Rc;
        struct Probe {
            disabled: bool,
            selected: bool,
            input: Entity<InputState>,
            button_color: Rc<RefCell<Option<Hsla>>>,
            input_color: Rc<RefCell<Option<Hsla>>>,
        }
        fn paint_color(target: Rc<RefCell<Option<Hsla>>>) -> impl IntoElement {
            canvas(
                |_, _, _| (),
                move |_, _, window, _| {
                    target.replace(Some(window.text_style().color));
                },
            )
            .size(px(12.))
        }
        impl Render for Probe {
            fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .v_flex()
                    .gap_4()
                    .p_4()
                    .child(
                        super::button("css-state-probe", cx)
                            .debug_selector(|| "css-state-probe".into())
                            .label("Probe")
                            .disabled(self.disabled)
                            .selected(self.selected)
                            .on_click(|_, _, _| {})
                            .child(paint_color(self.button_color.clone())),
                    )
                    .child(
                        super::input(&self.input, cx).prefix(paint_color(self.input_color.clone())),
                    )
            }
        }
        cx.update(gpui_component::init);
        cx.update(|cx| {
            cx.set_global(NativeStyles::new(
                Stylesheet::parse(["
            #css-state-probe { color: #101010; }
            #css-state-probe:hover { color: #202020; }
            #css-state-probe:hover:active { color: #303030; }
            #css-state-probe:checked { color: #505050; }
            #css-state-probe:disabled { color: #404040; }
            input { color: #707070; }
            input:focus { color: #606060; }
        "])
                .unwrap(),
                appearance::Palette::default(),
            ))
        });
        let button_color = Rc::new(RefCell::new(None));
        let input_color = Rc::new(RefCell::new(None));
        let view = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx));
            let probe = cx.new(|_| Probe {
                disabled: false,
                selected: false,
                input,
                button_color: button_color.clone(),
                input_color: input_color.clone(),
            });
            view.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = view.borrow_mut().take().unwrap();
        let expected = |hex| super::color(hex, &appearance::Palette::default()).unwrap();
        cx.run_until_parked();
        assert_eq!(*button_color.borrow(), Some(expected("#101010")));
        assert_eq!(*input_color.borrow(), Some(expected("#707070")));
        let button_point = cx.debug_bounds("css-state-probe").unwrap().center();
        cx.simulate_mouse_move(button_point, None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(*button_color.borrow(), Some(expected("#202020")));
        cx.simulate_mouse_down(button_point, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(*button_color.borrow(), Some(expected("#303030")));
        cx.simulate_mouse_up(button_point, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(point(px(700.), px(500.)), None, Modifiers::none());
        cx.update(|_, cx| {
            probe.update(cx, |probe, cx| {
                probe.selected = true;
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(*button_color.borrow(), Some(expected("#505050")));
        cx.update(|_, cx| {
            probe.update(cx, |probe, cx| {
                probe.disabled = true;
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(*button_color.borrow(), Some(expected("#404040")));
        cx.update(|window, cx| {
            probe.update(cx, |probe, cx| {
                probe.input.update(cx, |input, cx| input.focus(window, cx));
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(*input_color.borrow(), Some(expected("#606060")));
    }
    #[::core::prelude::v1::test]
    fn native_css_resolves_original_sizes_app_variables_and_scoped_overrides() {
        let mut palette = appearance::builtins()["defaultTheme"].clone();
        palette.mantine_override = serde_json::json!({"spacing":{"md":"2rem"}});
        let sheet = Stylesheet::parse(["
            body { --mantine-spacing-sm: 10px; }
            #theme-vars { padding: var(--theme-spacing-sm) var(--theme-spacing-md); max-width: var(--theme-content-max-width); font-size: var(--theme-font-size-sm); line-height: var(--theme-line-height-md); border-radius: var(--theme-radius-md); box-shadow: var(--theme-shadow-xs); }
            #scoped-vars { --theme-spacing-md: 3px; padding: var(--theme-spacing-md); }
        "]).unwrap();
        let styles = NativeStyles::new(sheet, palette);
        let style = styles.style(Target::new("div", "theme-vars", &[]), None, 16.);
        assert_eq!(style.padding.top, Some(px(10.).into()));
        assert_eq!(style.padding.left, Some(px(32.).into()));
        assert_eq!(style.max_size.width, Some(px(1800.).into()));
        assert_eq!(style.text.as_ref().unwrap().font_size, Some(px(13.).into()));
        assert_eq!(
            style.text.as_ref().unwrap().line_height,
            Some(px(18.).into())
        );
        assert_eq!(style.corner_radii.top_left, Some(px(5.).into()));
        assert_eq!(style.box_shadow.unwrap()[0].blur_radius, px(2.));
        let scoped = styles.style(Target::new("div", "scoped-vars", &[]), None, 16.);
        assert_eq!(scoped.padding.left, Some(px(3.).into()));
    }
    #[gpui::test]
    fn css_transforms_reach_images_svg_and_canvas(cx: &mut TestAppContext) {
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        struct Probe {
            style: StyleRefinement,
            image: std::sync::Arc<RenderImage>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .relative()
                    .size_full()
                    .child(
                        img(self.image.clone())
                            .absolute()
                            .left(px(40.))
                            .top(px(50.))
                            .w(px(120.))
                            .h(px(60.))
                            .refine_style(&self.style)
                            .bg(rgb(0x123456)),
                    )
                    .child(
                        svg()
                            .path("native-icons/play.svg")
                            .absolute()
                            .left(px(200.))
                            .top(px(50.))
                            .w(px(120.))
                            .h(px(60.))
                            .refine_style(&self.style)
                            .bg(rgb(0x234567)),
                    )
                    .child(
                        canvas(|_, _, _| (), |_, _, _, _| ())
                            .absolute()
                            .left(px(360.))
                            .top(px(50.))
                            .w(px(120.))
                            .h(px(60.))
                            .refine_style(&self.style)
                            .bg(rgb(0x345678)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(520.))
                            .top(px(50.))
                            .w(px(120.))
                            .h(px(60.))
                            .refine_style(&self.style)
                            .bg(rgb(0x456789))
                            .child("Transformed text"),
                    )
            }
        }
        cx.set_assets(crate::icons::Assets);
        let styles = NativeStyles::new(
            Stylesheet::parse([
                ".probe {transform:translate(12px, -7px) scale(1.2);transform-origin:10px 20px}",
            ])
            .unwrap(),
            appearance::Palette::default(),
        );
        let (probe, cx) = cx.add_window_view(|_, _| Probe {
            style: styles.style(Target::new("img", "", &["probe"]), None, 16.),
            image: std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(
                image::RgbaImage::from_pixel(2, 1, image::Rgba([255, 0, 0, 255])),
            )])),
        });
        cx.run_until_parked();
        let scale = cx.update(|window, _| window.scale_factor());
        let quads = cx.update(|window, _| window.painted_lines_and_quads_for_test());
        for (index, color) in [0x123456, 0x234567, 0x345678, 0x456789]
            .into_iter()
            .enumerate()
        {
            let color: Hsla = rgb(color).into();
            let bounds = quads
                .iter()
                .find(|(_, actual, _, _)| *actual == color)
                .unwrap()
                .0;
            for (actual, expected) in [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ]
            .into_iter()
            .zip([50. + index as f32 * 160., 39., 144., 72.])
            {
                assert!(
                    (f64::from(actual) as f32 / scale - expected).abs() < 0.05,
                    "Element {index}: {bounds:?}"
                );
            }
        }
        let images = cx.update(|window, _| window.painted_images_for_test());
        assert_eq!(images.len(), 1);
        for (actual, expected) in [
            images[0].0.origin.x,
            images[0].0.origin.y,
            images[0].0.size.width,
            images[0].0.size.height,
        ]
        .into_iter()
        .zip([50., 39., 144., 72.])
        {
            assert!((f32::from(actual) - expected).abs() < 0.05);
        }
        assert!(
            !cx.update(|window, _| window.painted_glyphs_for_test())
                .is_empty()
        );
        probe.update(cx, |probe, cx| {
            probe.style.paint_scale = Some(0.);
            cx.notify();
        });
        cx.run_until_parked();
        assert!(
            cx.update(|window, _| window.painted_lines_and_quads_for_test())
                .is_empty()
        );
        assert!(
            cx.update(|window, _| window.painted_images_for_test())
                .is_empty()
        );
        assert!(
            cx.update(|window, _| window.painted_glyphs_for_test())
                .is_empty()
        );
    }

    #[gpui::test]
    fn css_transforms_match_original_geometry_without_changing_layout(cx: &mut TestAppContext) {
        use std::{cell::RefCell, rc::Rc};
        struct Probe {
            clicks: usize,
            style: StyleRefinement,
            parent_style: StyleRefinement,
            zoom: f32,
            measured: Rc<RefCell<[[f32; 4]; 3]>>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let capture = |index: usize| {
                    let measured = self.measured.clone();
                    canvas(
                        move |bounds, window, _| {
                            let bounds = window.transform_element_bounds(bounds);
                            measured.borrow_mut()[index] = [
                                bounds.origin.x.into(),
                                bounds.origin.y.into(),
                                bounds.size.width.into(),
                                bounds.size.height.into(),
                            ];
                        },
                        |_, _, _, _| (),
                    )
                    .absolute()
                    .inset_0()
                    .size_full()
                };
                let zoom = self.zoom;
                div()
                    .relative()
                    .w(px(800. * zoom))
                    .h(px(600. * zoom))
                    .child(
                        div()
                            .absolute()
                            .v_flex()
                            .left(px(40. * zoom))
                            .top(px(50. * zoom))
                            .w(px(400. * zoom))
                            .h(px(300. * zoom))
                            .refine_style(&self.parent_style)
                            .child(
                                div()
                                    .relative()
                                    .w(px(120. * zoom))
                                    .h(px(60. * zoom))
                                    .flex_shrink_0()
                                    .refine_style(&self.style)
                                    .id("transform-probe")
                                    .role(Role::Button)
                                    .aria_label("Transform probe")
                                    .bg(rgb(0x123456))
                                    .on_click(cx.listener(|probe, _, _, cx| {
                                        probe.clicks += 1;
                                        cx.notify();
                                    }))
                                    .child(capture(0))
                                    .child(
                                        div()
                                            .absolute()
                                            .left(px(10. * zoom))
                                            .top(px(12. * zoom))
                                            .w(px(20. * zoom))
                                            .h(px(10. * zoom))
                                            .child(capture(1)),
                                    ),
                            )
                            .child(
                                div()
                                    .relative()
                                    .w(px(30. * zoom))
                                    .h(px(20. * zoom))
                                    .flex_shrink_0()
                                    .child(capture(2)),
                            ),
                    )
            }
        }
        let measured = Rc::new(RefCell::new([[0.; 4]; 3]));
        let (probe, cx) = cx.add_window_view(|window, _| {
            window.activate_accessibility_for_test();
            Probe {
                clicks: 0,
                style: Default::default(),
                parent_style: Default::default(),
                zoom: 1.,
                measured: measured.clone(),
            }
        });
        cx.simulate_resize(size(px(1200.), px(900.)));
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/css-transforms-reference.json")).unwrap();
        for case in cases {
            for zoom in [1., 1.5] {
                let source = format!(
                    "#parent {{{}}} .probe {{font-size:20px;{}}}",
                    case["parent"].as_str().unwrap(),
                    case["css"].as_str().unwrap()
                );
                let styles = NativeStyles::new(
                    Stylesheet::parse([source.as_str()]).unwrap(),
                    appearance::Palette::default(),
                )
                .with_zoom(zoom);
                if case["valid"].as_bool() != Some(false) {
                    assert!(styles.warnings.is_empty(), "{:?}", styles.warnings);
                }
                probe.update(cx, |probe, cx| {
                    probe.style = styles.style(
                        Target::new("div", "", &["probe"]),
                        Some(Target::new("div", "parent", &[])),
                        16. * zoom,
                    );
                    probe.parent_style =
                        styles.style(Target::new("div", "parent", &[]), None, 16. * zoom);
                    probe.zoom = zoom;
                    cx.notify();
                });
                cx.run_until_parked();
                let expected: [f32; 4] = std::array::from_fn(|axis| {
                    case["bounds"][axis].as_f64().unwrap() as f32 * zoom
                });
                let scale = cx.update(|window, _| window.scale_factor());
                let tree = cx
                    .update(|window, _| window.accessibility_tree_for_test().cloned())
                    .unwrap();
                let ax = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some("Transform probe"))
                    .unwrap()
                    .1
                    .bounds()
                    .unwrap();
                let rect = [ax.x0, ax.y0, ax.x1 - ax.x0, ax.y1 - ax.y0];
                for (actual, expected) in rect.into_iter().zip(expected) {
                    assert!(
                        (actual as f32 / scale - expected).abs() < 0.05,
                        "Accessibility: {case}"
                    );
                }
                let color: Hsla = rgb(0x123456).into();
                let quads = cx.update(|window, _| window.painted_lines_and_quads_for_test());
                let quad = quads.iter().find(|(_, actual, _, _)| *actual == color);
                let visible = expected[2] > 0. && expected[3] > 0.;
                if visible {
                    let quad = quad.unwrap().0;
                    for (actual, expected) in [
                        quad.origin.x,
                        quad.origin.y,
                        quad.size.width,
                        quad.size.height,
                    ]
                    .into_iter()
                    .zip(expected)
                    {
                        assert!(
                            (f64::from(actual) as f32 / scale - expected).abs() < 0.05,
                            "Painting: {case}"
                        );
                    }
                } else {
                    assert!(quad.is_none(), "Zero scale must not paint: {case}");
                }
                let clicks = probe.read_with(cx, |probe, _| probe.clicks);
                cx.simulate_click(
                    point(
                        px(expected[0] + expected[2] / 2.),
                        px(expected[1] + expected[3] / 2.),
                    ),
                    Modifiers::none(),
                );
                cx.run_until_parked();
                assert_eq!(
                    probe.read_with(cx, |probe, _| probe.clicks),
                    clicks + usize::from(visible),
                    "Hit target: {case}"
                );
                for (index, field) in ["bounds", "content", "sibling"].into_iter().enumerate() {
                    for (axis, actual) in measured.borrow()[index].into_iter().enumerate() {
                        let expected = case[field][axis].as_f64().unwrap() as f32 * zoom;
                        assert!(
                            (actual - expected).abs() < 0.05,
                            "{field}[{axis}]: {actual} != {expected}, zoom={zoom}: {case}"
                        );
                    }
                }
            }
        }
    }

    #[gpui::test]
    fn css_spacing_matches_original_percentages_logical_edges_and_cascade(cx: &mut TestAppContext) {
        struct Probe {
            style: StyleRefinement,
            parent_style: StyleRefinement,
            rtl: bool,
            zoom: f32,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .flex()
                    .w(px(800. * self.zoom))
                    .h(px(600. * self.zoom))
                    .when(self.rtl, |div| div.justify_end())
                    .child(
                        div()
                            .v_flex()
                            .flex_shrink_0()
                            .w(px(400. * self.zoom))
                            .h(px(300. * self.zoom))
                            .items_start()
                            .when(
                                self.parent_style
                                    .text
                                    .as_ref()
                                    .and_then(|text| text.text_direction)
                                    .map_or(self.rtl, |direction| direction == TextDirection::Rtl),
                                |div| div.items_end(),
                            )
                            .refine_style(&self.parent_style)
                            .child(
                                div()
                                    .id("spacing-probe")
                                    .debug_selector(|| "spacing-probe".into())
                                    .v_flex()
                                    .flex_shrink_0()
                                    .w(px(120. * self.zoom))
                                    .h(px(60. * self.zoom))
                                    .refine_style(&self.style)
                                    .child(
                                        div()
                                            .debug_selector(|| "spacing-content".into())
                                            .w_full()
                                            .h(px(10. * self.zoom))
                                            .flex_shrink_0(),
                                    ),
                            ),
                    )
            }
        }
        let (probe, cx) = cx.add_window_view(|_, _| Probe {
            style: Default::default(),
            parent_style: Default::default(),
            rtl: false,
            zoom: 1.,
        });
        cx.simulate_resize(size(px(1200.), px(900.)));
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/css-spacing-reference.json")).unwrap();
        for case in cases {
            for zoom in [1., 1.5] {
                let mut styles = NativeStyles::new(
                    Stylesheet::parse([case["source"].as_str().unwrap()]).unwrap(),
                    appearance::Palette::default(),
                )
                .with_zoom(zoom);
                styles.media.width = 800.;
                styles.media.height = 600.;
                if case["attribute"] == true {
                    styles.context[0]
                        .attributes
                        .insert("dir".into(), case["direction"].as_str().unwrap().into());
                }
                let style = styles.style(
                    Target::new("div", "", &["probe"]),
                    Some(Target::new("div", "parent", &[])),
                    16. * zoom,
                );
                for (kind, actual) in [
                    (
                        "margin",
                        [
                            style.margin.top,
                            style.margin.right,
                            style.margin.bottom,
                            style.margin.left,
                        ],
                    ),
                    (
                        "padding",
                        [
                            style.padding.top.map(Into::into),
                            style.padding.right.map(Into::into),
                            style.padding.bottom.map(Into::into),
                            style.padding.left.map(Into::into),
                        ],
                    ),
                ] {
                    for (index, actual) in actual.into_iter().enumerate() {
                        let value = case[kind][index].as_str().unwrap();
                        if value == "auto" {
                            assert_eq!(actual, Some(Length::Auto), "{case}");
                        } else {
                            let terms = &case[format!("{kind}Terms")][index];
                            let expected = (terms[0].as_f64().unwrap()
                                + terms[1].as_f64().unwrap() * 400.)
                                as f32
                                * zoom;
                            let Some(Length::Definite(actual)) = actual else {
                                panic!("Missing {kind}[{index}]: {case}");
                            };
                            let actual =
                                f32::from(actual.to_pixels(px(400. * zoom).into(), px(16. * zoom)));
                            let (actual, expected) = if kind == "padding" {
                                (actual.max(0.), expected.max(0.))
                            } else {
                                (actual, expected)
                            };
                            assert!(
                                (actual - expected).abs() < 0.0001,
                                "{kind}[{index}], {actual} != {expected}, zoom={zoom}: {case}"
                            );
                        }
                    }
                }
                assert!(styles.warnings.is_empty(), "{:?}", styles.warnings);
                probe.update(cx, |probe, cx| {
                    probe.style = style;
                    probe.parent_style =
                        styles.style(Target::new("div", "parent", &[]), None, 16. * zoom);
                    probe.rtl = case["direction"] == "rtl";
                    probe.zoom = zoom;
                    cx.notify();
                });
                cx.run_until_parked();
                for (selector, reference) in
                    [("spacing-probe", "bounds"), ("spacing-content", "content")]
                {
                    let bounds = cx.debug_bounds(selector).unwrap();
                    for (index, actual) in [
                        bounds.origin.x,
                        bounds.origin.y,
                        bounds.size.width,
                        bounds.size.height,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let expected = case[reference][index].as_f64().unwrap() as f32 * zoom;
                        assert!(
                            (f32::from(actual) - expected).abs() < 1.,
                            "{selector}[{index}], actual={actual:?}, expected={expected}, zoom={zoom}: {case}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn css_direction_inherits_resets_and_preserves_valid_declarations() {
        for (source, expected) in [
            ("", None),
            ("body { direction: rtl; }", Some(TextDirection::Rtl)),
            (
                "body { direction: rtl; } span { direction: inherit; }",
                Some(TextDirection::Rtl),
            ),
            (
                "body { direction: rtl; } span { direction: unset; }",
                Some(TextDirection::Rtl),
            ),
            (
                "body { direction: rtl; } span { direction: initial; }",
                Some(TextDirection::Ltr),
            ),
            (
                "body { direction: rtl; } span { direction: ltr; direction: bogus; }",
                Some(TextDirection::Ltr),
            ),
            (
                "body { --flow: rtl; } span { direction: var(--flow); }",
                Some(TextDirection::Rtl),
            ),
        ] {
            let styles = NativeStyles::new(
                Stylesheet::parse([source]).unwrap(),
                appearance::Palette::default(),
            );
            assert!(styles.warnings.is_empty(), "{:?}", styles.warnings);
            let actual = styles
                .style(Target::new("span", "direction-probe", &[]), None, 16.)
                .text
                .and_then(|style| style.text_direction);
            assert_eq!(actual, expected, "{source}");
        }
        for (align, expected) in [
            ("start", TextAlign::Start),
            ("end", TextAlign::End),
            ("left", TextAlign::Left),
            ("right", TextAlign::Right),
        ] {
            let styles = NativeStyles::new(
                Stylesheet::parse([format!("span {{ text-align: {align}; }}").as_str()]).unwrap(),
                appearance::Palette::default(),
            );
            assert_eq!(
                styles
                    .style(Target::new("span", "direction-probe", &[]), None, 16.)
                    .text
                    .and_then(|style| style.text_align),
                Some(expected)
            );
        }
    }

    #[test]
    fn css_font_styles_match_original_inheritance_resets_and_invalid_declarations() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/font-styles-reference.json")).unwrap();
        for platform in reference["platform_metrics"].as_object().unwrap().values() {
            for case in platform["cascades"].as_array().unwrap() {
                let source = case["source"].as_str().unwrap();
                let styles = NativeStyles::new(
                    Stylesheet::parse([source]).unwrap(),
                    appearance::Palette::default(),
                );
                let actual = styles
                    .style(Target::new("span", "style-probe", &[]), None, 16.)
                    .text
                    .and_then(|style| style.font_style)
                    .unwrap_or_default();
                let expected = match case["style"].as_str().unwrap() {
                    "italic" => FontStyle::Italic,
                    "oblique" => FontStyle::Oblique,
                    _ => FontStyle::Normal,
                };
                assert_eq!(actual, expected, "{source}");
            }
        }
    }

    #[test]
    fn css_font_variations_match_original_parser_and_cascade() {
        for source in [
            include_str!("../assets/font-variations-darwin-reference.json"),
            include_str!("../assets/font-variations-linux-reference.json"),
        ] {
            let reference: serde_json::Value = serde_json::from_str(source).unwrap();
            for case in reference["parsers"].as_array().unwrap() {
                let value = case["value"].as_str().unwrap();
                let actual = feishin_core::fonts::variation_settings(value);
                assert_eq!(
                    actual.is_some(),
                    case["valid"].as_bool().unwrap(),
                    "{value}"
                );
                if let Some(actual) = actual {
                    assert_eq!(
                        Some(actual),
                        feishin_core::fonts::variation_settings(case["computed"].as_str().unwrap()),
                        "{value}"
                    );
                }
            }
            for case in reference["cascades"].as_array().unwrap() {
                let source = case["source"].as_str().unwrap();
                let styles = NativeStyles::new(
                    Stylesheet::parse([source]).unwrap(),
                    appearance::Palette::default(),
                );
                assert!(styles.warnings.is_empty(), "{:?}", styles.warnings);
                let actual = styles
                    .style(Target::new("span", "variations", &[]), None, 16.)
                    .text
                    .and_then(|text| text.font_variations)
                    .unwrap_or_default();
                let expected = FontVariations::try_from(
                    feishin_core::fonts::variation_settings(case["computed"].as_str().unwrap())
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(actual, expected, "{source}");
            }
        }
        let canonical = FontVariations::try_from(vec![
            ("wght".into(), -0.),
            ("opsz".into(), 20.),
            ("wght".into(), 800.),
        ])
        .unwrap();
        assert_eq!(
            canonical,
            FontVariations::try_from(vec![("wght".into(), 800.), ("opsz".into(), 20.)]).unwrap()
        );
        let json = serde_json::to_string(&canonical).unwrap();
        assert_eq!(
            serde_json::from_str::<FontVariations>(&json).unwrap(),
            canonical
        );
        assert_eq!(
            FontVariations::try_from(vec![("wght".into(), -0.)]).unwrap(),
            FontVariations::try_from(vec![("wght".into(), 0.)]).unwrap()
        );
        assert!(FontVariations::try_from(vec![("wght".into(), f32::NAN)]).is_err());
    }

    #[gpui::test]
    fn native_font_variations_match_original_glyph_measurements(cx: &mut TestAppContext) {
        let system = cx.use_platform_text_system();
        system
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        let system = WindowTextSystem::new(system);
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/font-variations-linux-reference.json")
        } else {
            include_str!("../assets/font-variations-darwin-reference.json")
        })
        .unwrap();
        let mut rasters = BTreeMap::new();
        let mut fallback_rasters = BTreeMap::new();
        for case in reference["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .chain(reference["fallback_metrics"].as_array().unwrap())
        {
            let text = case["text"].as_str().unwrap();
            let size = case["size"].as_f64().unwrap() as f32;
            let style = TextStyle {
                font_family: case["family"].as_str().unwrap().to_owned().into(),
                font_fallbacks: case["fallback"]
                    .as_str()
                    .map(|family| FontFallbacks::from_fonts(vec![family.to_owned()])),
                font_weight: FontWeight(case["weight"].as_f64().unwrap() as f32),
                font_variations: FontVariations::try_from(
                    feishin_core::fonts::variation_settings(case["variations"].as_str().unwrap())
                        .unwrap(),
                )
                .unwrap(),
                ..Default::default()
            };
            let line = system.shape_line(
                text.to_owned().into(),
                px(size),
                &[style.to_run(text.len())],
                None,
            );
            let expected = case["width"].as_f64().unwrap() as f32;
            assert!(
                (f32::from(line.width) - expected).abs() < 0.08,
                "{case}: native width {}",
                f32::from(line.width)
            );
            if text == "H" {
                let layout = system.layout_line(text, px(size), &[style.to_run(text.len())], None);
                let run = &layout.runs[0];
                let raster = system
                    .rasterized_glyph_for_test(run.font_id, run.glyphs[0].id, px(size), 1.)
                    .unwrap();
                rasters.insert(
                    (
                        case["family"].as_str().unwrap(),
                        size as u32,
                        style.font_weight.0 as u32,
                        case["variations"].as_str().unwrap(),
                    ),
                    raster,
                );
            }
            if let Some(family) = case["fallback"].as_str() {
                let layout = system.layout_line(text, px(size), &[style.to_run(text.len())], None);
                let run = &layout.runs[0];
                let raster = system
                    .rasterized_glyph_for_test(run.font_id, run.glyphs[0].id, px(size), 1.)
                    .unwrap();
                fallback_rasters.insert(
                    (
                        family,
                        text,
                        size as u32,
                        style.font_weight.0 as u32,
                        case["variations"].as_str().unwrap(),
                    ),
                    raster,
                );
            }
            system.set_optical_size_scale(2.);
            let zoomed = system.shape_line(
                text.to_owned().into(),
                px(size * 2.),
                &[style.to_run(text.len())],
                None,
            );
            assert!(
                (f32::from(zoomed.width) - expected * 2.).abs() < 0.16,
                "Zoom changed variation coordinates: {case}: native {}",
                f32::from(zoomed.width)
            );
            system.set_optical_size_scale(1.);
        }
        for family in ["Inter", "Poppins"] {
            for size in [14, 20, 32] {
                for weight in [400, 700] {
                    let raster = |value| &rasters[&(family, size, weight, value)];
                    if family == "Inter" {
                        assert_ne!(
                            raster("\"wght\" 200"),
                            raster("\"wght\" 800"),
                            "Variable weight did not affect pixels"
                        );
                        assert_ne!(
                            raster("\"opsz\" 14"),
                            raster("\"opsz\" 32"),
                            "Optical size did not affect pixels"
                        );
                        assert_eq!(
                            raster("\"opsz\" 32"),
                            raster("\"opsz\" 200"),
                            "Optical size did not clamp"
                        );
                    } else {
                        assert_eq!(
                            raster("normal"),
                            raster("\"wght\" 800"),
                            "Override selected a different static face"
                        );
                    }
                    assert_eq!(raster("\"wght\" 800"), raster("\"wght\" 200, \"wght\" 800"));
                    assert_eq!(raster("normal"), raster("\"WGHT\" 900"));
                }
            }
        }
        for (family, text) in [
            ("Inter", "Ω"),
            ("Inter", "Ж"),
            ("Noto Sans JP", "日"),
            ("Noto Sans Hebrew", "ש"),
        ] {
            for size in [20, 32] {
                let raster = |weight, variations| {
                    &fallback_rasters[&(family, text, size, weight, variations)]
                };
                assert_ne!(
                    raster(400, "\"wght\" 200"),
                    raster(400, "\"wght\" 800"),
                    "Fallback pixels ignore variable weight: {family}"
                );
                assert_eq!(
                    raster(400, "\"wght\" 800"),
                    raster(700, "\"wght\" 800"),
                    "Fallback CSS weight overrode an explicit axis: {family}"
                );
                if family == "Noto Sans Hebrew" {
                    assert_ne!(raster(400, "normal"), raster(400, "\"wdth\" 75"));
                    assert_eq!(raster(400, "\"wdth\" 100"), raster(400, "\"wdth\" 125"));
                }
            }
        }
    }

    #[gpui::test]
    fn font_variations_reflow_existing_multiline_inputs(cx: &mut TestAppContext) {
        use gpui_component::{
            Root,
            input::{InputState, Position},
        };
        use std::{cell::RefCell, rc::Rc};
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        struct Probe {
            style: StyleRefinement,
            weight: f32,
            input: [Entity<InputState>; 2],
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .v_flex()
                    .font_family("Inter")
                    .children(self.input.iter().enumerate().map(|(index, input)| {
                        div()
                            .debug_selector(move || format!("variations-input-{index}"))
                            .w(px(98.))
                            .h(px(180.))
                            .when(index == 0, |div| div.refine_style(&self.style))
                            .when(index == 1, |div| div.font_weight(FontWeight(self.weight)))
                            .child(
                                Input::new(input)
                                    .bordered(false)
                                    .p_0()
                                    .h(px(180.))
                                    .text_size(px(20.))
                                    .line_height(relative(1.5)),
                            )
                    }))
            }
        }
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let input = std::array::from_fn(|_| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .multi_line(true)
                        .rows(6)
                        .default_value("HHHHHHHHHHHHHHHHHHHH")
                })
            });
            let probe = cx.new(|_| Probe {
                style: Default::default(),
                weight: 400.,
                input,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        let inputs = probe.read_with(cx, |view, _| view.input.clone());
        let mut positions = vec![];
        for weight in [200., 800., 400., 200.] {
            let source = format!("span {{ font-variation-settings: 'wght' {weight}; }}");
            let style = NativeStyles::new(
                Stylesheet::parse([source.as_str()]).unwrap(),
                Default::default(),
            )
            .style(Target::new("span", "", &[]), None, 16.);
            probe.update(cx, |view, cx| {
                view.weight = weight;
                view.style = style;
                cx.notify();
            });
            cx.run_until_parked();
            let mut carets = vec![];
            for (index, input) in inputs.iter().enumerate() {
                cx.update(|window, cx| {
                    window.activate_window();
                    window.focus(&input.read(cx).focus_handle(cx));
                    input.update(cx, |input, cx| {
                        input.set_cursor_position(Position::new(0, 20), window, cx)
                    });
                });
                cx.run_until_parked();
                let origin = cx
                    .debug_bounds(&format!("variations-input-{index}"))
                    .unwrap()
                    .origin;
                let caret = cx.update(|window, cx| {
                    window
                        .painted_lines_and_quads_for_test()
                        .into_iter()
                        .find(|(bounds, color, _, _)| {
                            *color == cx.theme().caret
                                && bounds.size.width
                                    <= ScaledPixels::from(2. * window.scale_factor())
                        })
                        .map(|(bounds, _, _, _)| {
                            point(
                                px(f64::from(bounds.origin.x) as f32 / window.scale_factor()),
                                px(f64::from(bounds.origin.y) as f32 / window.scale_factor()),
                            )
                        })
                        .expect("Focused variable-font input caret")
                });
                carets.push(caret - origin);
            }
            assert!(
                (carets[0].x - carets[1].x).abs() < px(0.1)
                    && (carets[0].y - carets[1].y).abs() < px(0.1),
                "{weight}: {carets:?}"
            );
            positions.push(carets[0]);
        }
        assert_ne!(
            positions[0], positions[1],
            "Input did not reflow after a variation change"
        );
        assert_eq!(
            positions[0], positions[3],
            "Restored axes reused stale cached wrapping"
        );
    }

    #[::core::prelude::v1::test]
    fn css_wrap_modes_inherit_and_preserve_valid_values() {
        for (source, expected) in [
            ("", None),
            ("body { word-break: break-all; }", Some(TextWrap::BreakAll)),
            ("body { word-break: keep-all; }", Some(TextWrap::KeepAll)),
            (
                "body { --wrap: keep-all; word-break: var(--wrap); } span { word-break: inherit; }",
                Some(TextWrap::KeepAll),
            ),
            (
                "body { word-break: keep-all; overflow-wrap: break-word; }",
                Some(TextWrap::KeepAllBreakWord),
            ),
            (
                "body { word-break: keep-all; overflow-wrap: anywhere; }",
                Some(TextWrap::KeepAllBreakWord),
            ),
            (
                "body { word-break: keep-all; overflow-wrap: break-word; } span { word-break: normal; }",
                Some(TextWrap::BreakWord),
            ),
            (
                "body { word-break: break-all; } span { word-break: inherit; }",
                Some(TextWrap::BreakAll),
            ),
            (
                "body { word-break: break-all; } span { word-break: unset; }",
                Some(TextWrap::BreakAll),
            ),
            (
                "body { word-break: break-all; } span { word-break: initial; }",
                Some(TextWrap::Normal),
            ),
            (
                "body { overflow-wrap: break-word; } span { word-break: normal; }",
                Some(TextWrap::BreakWord),
            ),
            (
                "body { word-break: break-all; overflow-wrap: break-word; }",
                Some(TextWrap::BreakAll),
            ),
            (
                "body { --wrap: anywhere; } span { overflow-wrap: var(--wrap); }",
                Some(TextWrap::BreakAll),
            ),
            (
                "span { overflow-wrap: break-word; overflow-wrap: bogus; }",
                Some(TextWrap::BreakWord),
            ),
            (
                "body { overflow-wrap: break-word; } span { overflow-wrap: initial; }",
                Some(TextWrap::Normal),
            ),
        ] {
            let styles = NativeStyles::new(
                Stylesheet::parse([source]).unwrap(),
                appearance::Palette::default(),
            );
            assert!(styles.warnings.is_empty(), "{:?}", styles.warnings);
            assert_eq!(
                styles
                    .style(Target::new("span", "wrap-probe", &[]), None, 16.)
                    .text
                    .and_then(|style| style.text_wrap),
                expected,
                "{source}"
            );
        }
    }

    #[gpui::test]
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "freebsd"))]
    fn multiline_inputs_preserve_original_wrapped_rows_and_clicked_edges(cx: &mut TestAppContext) {
        use gpui_component::{
            Root,
            input::{InputState, RopeExt as _},
        };
        use std::{borrow::Cow, rc::Rc};
        struct Probe {
            input: Entity<InputState>,
            direction: TextDirection,
            width: f32,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .w(px(self.width + 10.))
                    .h(px(500.))
                    .text_direction(self.direction)
                    .text_align(TextAlign::Start)
                    .text_spacing_trim(TextSpacingTrim::Normal)
                    .font_features(FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)])))
                    .font_fallbacks(FontFallbacks::from_fonts(vec![
                        "Noto Sans JP".into(),
                        "Noto Sans Hebrew".into(),
                    ]))
                    .child(
                        Input::new(&self.input)
                            .bordered(false)
                            .h(px(500.))
                            .p_0()
                            .font_family("Inter")
                            .text_size(px(14.))
                            .line_height(relative(1.5)),
                    )
            }
        }
        let system = cx.use_platform_text_system();
        system
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        let system = WindowTextSystem::new(system);
        cx.update(gpui_component::init);
        let view = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx).multi_line(true).rows(20));
            let probe = cx.new(|_| Probe {
                input,
                direction: TextDirection::Ltr,
                width: 42.,
            });
            view.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = view.borrow_mut().take().unwrap();
        let input = cx.update(|_, cx| probe.read(cx).input.clone());
        cx.update(|window, _| window.activate_window());
        let painted_caret = |cx: &mut VisualTestContext| {
            cx.update(|window, cx| {
                window
                    .painted_lines_and_quads_for_test()
                    .into_iter()
                    .find(|(bounds, color, _, _)| {
                        *color == cx.theme().caret
                            && bounds.size.width <= ScaledPixels::from(2. * window.scale_factor())
                    })
                    .map(|(bounds, _, _, _)| {
                        point(
                            px(f64::from(bounds.origin.x) as f32 / window.scale_factor()),
                            px(f64::from(bounds.origin.y + bounds.size.height / 2.) as f32
                                / window.scale_factor()),
                        )
                    })
                    .expect("Focused multiline input cursor")
            })
        };
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/font-spacing-word-wraps-electron-reference.json"
        ))
        .unwrap();
        for case in reference["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| {
                case["families"][0] == "Inter"
                    && case["size"] == 14
                    && case["wrap_mode"] == "break-word"
            })
        {
            let text = case["text"].as_str().unwrap();
            let width = case["wrap"].as_f64().unwrap() as f32;
            let direction = if case["direction"] == "rtl" {
                TextDirection::Rtl
            } else {
                TextDirection::Ltr
            };
            cx.update(|window, cx| {
                probe.update(cx, |probe, cx| {
                    probe.width = width;
                    probe.direction = direction;
                    cx.notify();
                });
                input.update(cx, |input, cx| {
                    input.set_value(text.to_owned(), window, cx);
                    input.set_cursor_position(
                        gpui_component::input::Position::new(0, 0),
                        window,
                        cx,
                    );
                });
            });
            cx.run_until_parked();
            let style = TextStyle {
                font_family: "Inter".into(),
                font_fallbacks: Some(FontFallbacks::from_fonts(vec![
                    "Noto Sans JP".into(),
                    "Noto Sans Hebrew".into(),
                ])),
                text_direction: direction,
                letter_spacing: gpui::px(0.),
                text_spacing_trim: TextSpacingTrim::Normal,
                font_features: FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)])),
                ..Default::default()
            };
            let shaped = system
                .shape_text_with_wrap(
                    text.to_owned().into(),
                    px(14.),
                    &[style.to_run(text.len())],
                    Some(px(width)),
                    None,
                    TextWrap::BreakWord,
                )
                .unwrap();
            let rows = shaped[0].shaped_rows();
            let first = &rows[0];
            let first_alignment = TextFragment {
                range: 0..first.len(),
                x: px(0.),
                width: first.width,
            }
            .alignment_offset(TextAlign::Start, px(width), first.rtl);
            cx.update(|window, cx| {
                input.update(cx, |input, cx| {
                    input.set_cursor_position(
                        gpui_component::input::Position::new(0, 0),
                        window,
                        cx,
                    )
                })
            });
            cx.run_until_parked();
            let initial = painted_caret(cx);
            let origin = point(
                initial.x - first_alignment - first.caret_x(0, TextAffinity::Editing),
                initial.y,
            );
            let mut offset = 0;
            for (row_index, row) in rows.iter().enumerate() {
                let aligned = TextFragment {
                    range: 0..row.len(),
                    x: px(0.),
                    width: row.width,
                }
                .alignment_offset(TextAlign::Start, px(width), row.rtl);
                for cell in &row.cells {
                    if (cell.end_x - cell.start_x).abs() <= px(0.01) {
                        continue;
                    }
                    for (fraction, source, edge) in [
                        (0.1, cell.range.start, cell.start_x),
                        (0.9, cell.range.end, cell.end_x),
                    ] {
                        let point = point(
                            origin.x
                                + aligned
                                + cell.start_x
                                + (cell.end_x - cell.start_x) * fraction,
                            origin.y + px(21.) * row_index,
                        );
                        if point.x < origin.x || point.x > origin.x + px(width) {
                            continue;
                        }
                        cx.simulate_click(point, Modifiers::none());
                        cx.run_until_parked();
                        assert_eq!(
                            cx.update(|_, cx| input.read(cx).cursor()),
                            offset + source,
                            "Multiline hit mismatch: {case}, row {row_index}, {cell:?}"
                        );
                        let cursor = painted_caret(cx);
                        assert!(
                            (cursor.x - origin.x - aligned - edge).abs() < px(0.05)
                                && (cursor.y - point.y).abs() < px(0.05),
                            "Multiline caret changed its wrapped edge: {case}, row {row_index}, {cell:?}, {cursor:?}, {point:?}"
                        );
                        let candidate = cx.update(|window, cx| {
                            input.update(cx, |input, cx| {
                                let range = EntityInputHandler::selected_text_range(
                                    input, false, window, cx,
                                )
                                .unwrap()
                                .range;
                                EntityInputHandler::bounds_for_range(
                                    input,
                                    range,
                                    Bounds::default(),
                                    window,
                                    cx,
                                )
                                .unwrap()
                            })
                        });
                        assert!(
                            (candidate.left() - cursor.x).abs() < px(0.05)
                                && (candidate.top() + px(10.5) - cursor.y).abs() < px(0.05)
                                && candidate.size.width >= px(0.),
                            "IME candidate moved off its clicked cursor: {case}, {candidate:?}, {cursor:?}"
                        );
                    }
                }
                offset += row.len();
            }
        }
        let platform = if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        };
        let mut differences = Vec::new();
        for (edit_kind, reference) in [
            (
                0,
                include_str!("../assets/input-multiline-electron-reference.json"),
            ),
            (
                1,
                include_str!("../assets/input-after-edit-electron-reference.json"),
            ),
            (
                2,
                include_str!("../assets/input-after-edit-electron-reference.json"),
            ),
        ] {
            let editing: serde_json::Value = serde_json::from_str(reference).unwrap();
            let cases = editing["platform_metrics"][platform]
                .as_array()
                .unwrap_or_else(|| editing["metrics"].as_array().unwrap());
            for case in cases {
                let text = case["text"].as_str().unwrap();
                cx.update(|window, cx| {
                    probe.update(cx, |probe, cx| {
                        probe.width = case["width"].as_f64().unwrap() as f32;
                        probe.direction = if case["direction"] == "rtl" {
                            TextDirection::Rtl
                        } else {
                            TextDirection::Ltr
                        };
                        cx.notify();
                    });
                    input.update(cx, |input, cx| input.set_value(text.to_owned(), window, cx));
                });
                cx.run_until_parked();
                for action in case["actions"].as_array().unwrap() {
                    if edit_kind != 0 {
                        cx.update(|window, cx| {
                            input.update(cx, |input, cx| input.set_value("", window, cx));
                        });
                        cx.run_until_parked();
                        cx.update(|window, cx| {
                            input.update(cx, |input, cx| {
                                if edit_kind == 2 {
                                    EntityInputHandler::replace_and_mark_text_in_range(
                                        input, None, text, None, window, cx,
                                    )
                                } else {
                                    EntityInputHandler::replace_text_in_range(
                                        input, None, text, window, cx,
                                    )
                                }
                            });
                        });
                    } else {
                        cx.update(|window, cx| {
                            input.update(cx, |input, cx| {
                                input.set_cursor_position(
                                    input.text().offset_to_position(
                                        input.text().offset_utf16_to_offset(
                                            action["start"].as_u64().unwrap() as usize,
                                        ),
                                    ),
                                    window,
                                    cx,
                                )
                            })
                        });
                    }
                    cx.run_until_parked();
                    cx.simulate_keystrokes(&action["key"].as_str().unwrap().to_ascii_lowercase());
                    let selected = cx.update(|window, cx| {
                        input.update(cx, |input, cx| {
                            EntityInputHandler::selected_text_range(input, false, window, cx)
                                .unwrap()
                                .range
                        })
                    });
                    let expected = action["selection"][0].as_u64().unwrap() as usize
                        ..action["selection"][1].as_u64().unwrap() as usize;
                    if selected != expected {
                        differences.push(format!(
                        "edit_kind={edit_kind} {} {}px {}: {} at {} -> {selected:?}, expected {expected:?}",
                        case["direction"], case["width"], text, action["key"], action["start"]
                    ));
                    }
                }
            }
        }
        assert!(
            differences.is_empty(),
            "Original multiline navigation differs in {} cases: {:?}",
            differences.len(),
            &differences[..differences.len().min(16)]
        );
    }

    #[gpui::test]
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "freebsd"))]
    fn bidi_inputs_match_original_arrows_and_preserve_clicked_edges(cx: &mut TestAppContext) {
        use gpui_component::{Root, input::InputState};
        use std::{borrow::Cow, rc::Rc};
        struct Probe {
            input: Entity<InputState>,
            direction: TextDirection,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
                    .w(px(500.))
                    .p_4()
                    .text_direction(self.direction)
                    .text_align(TextAlign::Start)
                    .font_fallbacks(FontFallbacks::from_fonts(vec!["Noto Sans Hebrew".into()]))
                    .child(
                        Input::new(&self.input)
                            .font_family("Inter")
                            .text_size(px(24.)),
                    )
            }
        }
        let system = cx.use_platform_text_system();
        system
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        let system = WindowTextSystem::new(system);
        cx.update(gpui_component::init);
        let view = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            let input = cx.new(|cx| InputState::new(window, cx));
            let probe = cx.new(|_| Probe {
                input,
                direction: TextDirection::Ltr,
            });
            view.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = view.borrow_mut().take().unwrap();
        let input = cx.update(|_, cx| probe.read(cx).input.clone());
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let painted_caret = |cx: &mut VisualTestContext| {
            cx.update(|window, cx| {
                let color = cx.theme().caret;
                window
                    .painted_lines_and_quads_for_test()
                    .into_iter()
                    .find(|(bounds, actual, _, _)| {
                        *actual == color && bounds.size.width <= ScaledPixels::from(2. * window.scale_factor())
                    })
                    .map(|(bounds, _, _, _)| {
                        point(
                            px(f64::from(bounds.origin.x) as f32 / window.scale_factor()),
                            px(f64::from(bounds.origin.y + bounds.size.height / 2.) as f32 / window.scale_factor()),
                        )
                    })
                    .unwrap_or_else(|| panic!("Focused input should paint its cursor: active={}, focused={}, primitives={:?}", window.is_window_active(), input.read(cx).focus_handle(cx).is_focused(window), window.painted_lines_and_quads_for_test()))
            })
        };
        for direction in [TextDirection::Ltr, TextDirection::Rtl] {
            for text in ["ABC אבג DEF", "שָׁלוֹם a\u{0308}b", "a\u{0308}b"] {
                cx.update(|window, cx| {
                    probe.update(cx, |probe, cx| {
                        probe.direction = direction;
                        cx.notify();
                    });
                    input.update(cx, |input, cx| {
                        input.set_value(text, window, cx);
                        input.set_cursor_position(
                            gpui_component::input::Position::new(0, 0),
                            window,
                            cx,
                        );
                    });
                });
                cx.run_until_parked();
                let style = TextStyle {
                    font_family: "Inter".into(),
                    font_fallbacks: Some(FontFallbacks::from_fonts(vec![
                        "Noto Sans Hebrew".into(),
                    ])),
                    text_direction: direction,
                    ..Default::default()
                };
                let layout = system.layout_line(text, px(24.), &[style.to_run(text.len())], None);
                let initial = painted_caret(cx);
                let origin = initial.x - layout.caret_x(0, TextAffinity::Editing);
                for cell in &layout.cells {
                    if (cell.end_x - cell.start_x).abs() <= px(0.01) {
                        continue;
                    }
                    for (fraction, expected, edge) in [
                        (0.1, cell.range.start, cell.start_x),
                        (0.9, cell.range.end, cell.end_x),
                    ] {
                        cx.simulate_click(
                            point(
                                origin + cell.start_x + (cell.end_x - cell.start_x) * fraction,
                                initial.y,
                            ),
                            Modifiers::none(),
                        );
                        cx.run_until_parked();
                        assert_eq!(
                            cx.update(|_, cx| input.read(cx).cursor()),
                            expected,
                            "{text:?}: {cell:?}"
                        );
                        assert!(
                            (painted_caret(cx).x - origin - edge).abs() < px(0.05),
                            "Painted cursor lost bidi affinity: {text:?}, {cell:?}"
                        );
                    }
                }
                let reference: serde_json::Value = serde_json::from_str(include_str!(
                    "../assets/input-editing-electron-reference.json"
                ))
                .unwrap();
                let case = reference["metrics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|case| {
                        case["text"] == text
                            && case["direction"]
                                == if direction == TextDirection::Rtl {
                                    "rtl"
                                } else {
                                    "ltr"
                                }
                    })
                    .unwrap();
                for action in case["actions"].as_array().unwrap() {
                    cx.update(|window, cx| {
                        input.update(cx, |input, cx| {
                            input.set_cursor_position(
                                gpui_component::input::Position::new(
                                    0,
                                    action["start"].as_u64().unwrap() as u32,
                                ),
                                window,
                                cx,
                            );
                        })
                    });
                    cx.run_until_parked();
                    cx.simulate_keystrokes(if action["key"] == "Left" {
                        "left"
                    } else {
                        "right"
                    });
                    cx.run_until_parked();
                    let selection = cx.update(|window, cx| {
                        input.update(cx, |input, cx| {
                            input.selected_text_range(false, window, cx).unwrap().range
                        })
                    });
                    assert_eq!(
                        selection,
                        action["selection"][0].as_u64().unwrap() as usize
                            ..action["selection"][1].as_u64().unwrap() as usize,
                        "Original input keyboard mismatch for {text:?}: {action}"
                    );
                }
                if text == "ABC אבג DEF" {
                    let start = if direction == TextDirection::Rtl {
                        2
                    } else {
                        3
                    };
                    cx.update(|window, cx| {
                        input.update(cx, |input, cx| {
                            input.set_cursor_position(
                                gpui_component::input::Position::new(0, start),
                                window,
                                cx,
                            )
                        })
                    });
                    cx.run_until_parked();
                    let key = if direction == TextDirection::Rtl {
                        "shift-left"
                    } else {
                        "shift-right"
                    };
                    cx.simulate_keystrokes(key);
                    cx.simulate_keystrokes(key);
                    cx.run_until_parked();
                    let end = text.char_indices().nth(start as usize + 2).unwrap().0;
                    let spans = layout.selection_ranges(start as usize..end);
                    assert_eq!(
                        spans.len(),
                        2,
                        "Mixed-direction selection must have a visual gap"
                    );
                    let vertices = cx.update(|window, cx| {
                        window
                            .painted_path_vertices_for_test()
                            .into_iter()
                            .find(|(color, _)| *color == cx.theme().selection)
                            .unwrap()
                            .1
                    });
                    let top = vertices.iter().map(|p| p.y).min().unwrap();
                    let bottom = vertices.iter().map(|p| p.y).max().unwrap();
                    let area = vertices
                        .chunks_exact(3)
                        .map(|triangle| {
                            let a = triangle[1] - triangle[0];
                            let b = triangle[2] - triangle[0];
                            (f32::from(a.x) * f32::from(b.y) - f32::from(a.y) * f32::from(b.x))
                                .abs()
                                / 2.
                        })
                        .sum::<f32>();
                    let expected = spans
                        .iter()
                        .map(|span| f32::from(span.end - span.start))
                        .sum::<f32>();
                    assert!(
                        (area / f32::from(bottom - top) - expected).abs() < 0.05,
                        "Selection paint bridged a bidi gap: {direction:?}"
                    );
                }
            }
        }
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.set_value("a\u{0308}b", window, cx);
                input.set_cursor_position(gpui_component::input::Position::new(0, 3), window, cx);
            })
        });
        cx.simulate_keystrokes("backspace");
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| input.read(cx).value()), "a\u{0308}");
        cx.simulate_keystrokes("backspace");
        cx.run_until_parked();
        // Original Electron removes a combining mark separately on Backspace.
        assert_eq!(cx.update(|_, cx| input.read(cx).value()), "a");
        cx.simulate_keystrokes("backspace");
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| input.read(cx).value()), "");
    }

    #[gpui::test]
    fn native_css_text_decoration_patterns_reach_wrapped_painted_lines(cx: &mut TestAppContext) {
        struct Probe;
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                super::apply(
                    div()
                        .w(px(160.))
                        .text_size(px(18.))
                        .child("Text decorations wrap onto several lines"),
                    "div",
                    "decoration-probe",
                    &[],
                    None,
                    cx,
                )
            }
        }
        cx.update(gpui_component::init);
        for pattern in ["solid", "dotted", "dashed", "double", "wavy", "none"] {
            cx.update(|cx| cx.set_global(NativeStyles::new(
                Stylesheet::parse([if pattern == "none" {
                    "#decoration-probe { text-decoration: none; }".to_owned()
                } else {
                    format!("#decoration-probe {{ text-decoration: underline overline line-through {pattern} #112233 2px; }}")
                }.as_str()]).unwrap(),
                appearance::Palette::default(),
            )));
            let (_, cx) = cx.add_window_view(|_, _| Probe);
            cx.run_until_parked();
            let expected: Hsla = rgb(0x112233).into();
            let primitives = cx
                .update(|window, _| window.painted_lines_and_quads_for_test())
                .into_iter()
                .filter(|(_, color, _, _)| *color == expected)
                .collect::<Vec<_>>();
            if pattern == "none" {
                assert!(primitives.is_empty());
                continue;
            }
            assert!(primitives.len() >= 4, "{pattern}: {primitives:?}");
            assert!(
                primitives.iter().all(|(bounds, _, rounded, wavy)| {
                    bounds.size.width > ScaledPixels::from(0.)
                        && bounds.size.height > ScaledPixels::from(0.)
                        && *rounded == (pattern == "dotted")
                        && *wavy == (pattern == "wavy")
                }),
                "{pattern}: {primitives:?}"
            );
            if pattern == "double" {
                assert_eq!(primitives.len() % 6, 0);
            }
        }
    }
    #[::core::prelude::v1::test]
    fn native_css_font_features_inherit_and_low_level_settings_override_variants() {
        let features = |source: &str, child: bool| {
            let styles = NativeStyles::new(
                Stylesheet::parse([source]).unwrap(),
                appearance::Palette::default(),
            );
            assert!(styles.warnings.is_empty(), "{:?}", styles.warnings);
            let target = if child {
                Target::new("span", "font-probe", &[])
            } else {
                Target::new("body", "native-root", &[])
            };
            styles
                .style(target, None, 16.)
                .text
                .and_then(|text| text.font_features)
                .map(|features| features.0.as_ref().clone())
        };
        assert_eq!(features("", true), None);
        for source in [
            "body { font-variant-numeric: tabular-nums slashed-zero; font-feature-settings: 'tnum' off; }",
            "body { font-feature-settings: 'tnum' off; font-variant-numeric: tabular-nums slashed-zero; }",
        ] {
            for child in [false, true] {
                assert_eq!(
                    features(source, child),
                    Some(vec![("tnum".into(), 0), ("zero".into(), 1)])
                );
            }
        }
        for reset in ["normal", "initial"] {
            let source = format!(
                "body {{ font-variant-numeric: tabular-nums; font-feature-settings: 'liga' off; }} span {{ font-variant-numeric: {reset}; font-feature-settings: {reset}; }}"
            );
            assert_eq!(features(&source, true), Some(vec![]));
        }
        for inherit in ["inherit", "unset", "revert"] {
            let source = format!(
                "body {{ font-variant-numeric: tabular-nums; font-feature-settings: 'liga' off; }} span {{ font-variant-numeric: proportional-nums; font-variant-numeric: {inherit}; font-feature-settings: {inherit}; }}"
            );
            assert_eq!(
                features(&source, true),
                Some(vec![("liga".into(), 0), ("tnum".into(), 1)])
            );
        }
        assert_eq!(
            features(
                "body { --numeric: tabular-nums; --features: 'liga' off; font-variant-numeric: var(--numeric); font-feature-settings: var(--features); } span { --numeric: proportional-nums; --features: 'zero' on; }",
                true
            ),
            Some(vec![("liga".into(), 0), ("tnum".into(), 1)])
        );
        assert_eq!(
            features(
                "body { font-variant-numeric: slashed-zero; font-feature-settings: 'liga' off; } span { font-variant-numeric: normal; font-feature-settings: 'tnum' on; }",
                true
            ),
            Some(vec![("tnum".into(), 1)])
        );
        assert_eq!(
            features(
                "body { font-variant-numeric: slashed-zero; font-variant-numeric: tabular-nums proportional-nums; font-feature-settings: 'liga' off; font-feature-settings: 'bad'; }",
                true
            ),
            Some(vec![("liga".into(), 0), ("zero".into(), 1)])
        );
        assert_eq!(
            features(
                "body { font-variant-numeric: normal !important; font-variant-numeric: slashed-zero; }",
                true
            ),
            Some(vec![])
        );
        assert_eq!(
            features("span { font-feature-settings: 'liga' off; }", true),
            Some(vec![("liga".into(), 0), ("tnum".into(), 1)])
        );
    }

    #[gpui::test]
    fn punctuation_spacing_reaches_nested_text_and_resolves_css_cascade(cx: &mut TestAppContext) {
        use std::{cell::RefCell, rc::Rc};
        struct Probe(Rc<RefCell<Option<TextSpacingTrim>>>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let actual = self.0.clone();
                div()
                    .text_spacing_trim(TextSpacingTrim::Normal)
                    .size_full()
                    .child(super::apply(
                        div().child(
                            canvas(
                                move |_, window, _| {
                                    *actual.borrow_mut() =
                                        Some(window.text_style().to_run(1).text_spacing_trim);
                                },
                                |_, _, _, _| {},
                            )
                            .size_full(),
                        ),
                        "span",
                        "spacing-probe",
                        &[],
                        None,
                        cx,
                    ))
            }
        }
        cx.update(gpui_component::init);
        for (source, expected) in [
            ("", TextSpacingTrim::Normal),
            (
                "body { text-spacing-trim: space-all; }",
                TextSpacingTrim::SpaceAll,
            ),
            (
                "body { text-spacing-trim: trim-start; } span { text-spacing-trim: inherit; }",
                TextSpacingTrim::TrimStart,
            ),
            (
                "body { text-spacing-trim: space-all; } span { text-spacing-trim: initial; }",
                TextSpacingTrim::Normal,
            ),
            (
                "body { --spacing: space-first; text-spacing-trim: var(--spacing); } span { --spacing: space-all; }",
                TextSpacingTrim::SpaceFirst,
            ),
            (
                "span { text-spacing-trim: trim-start; text-spacing-trim: bogus; }",
                TextSpacingTrim::TrimStart,
            ),
            (
                "span { text-spacing-trim: space-all !important; text-spacing-trim: trim-start; }",
                TextSpacingTrim::SpaceAll,
            ),
        ] {
            cx.update(|cx| {
                cx.set_global(NativeStyles::new(
                    Stylesheet::parse([source]).unwrap(),
                    appearance::Palette::default(),
                ))
            });
            let actual = Rc::new(RefCell::new(None));
            let (_, cx) = cx.add_window_view(|_, _| Probe(actual.clone()));
            cx.run_until_parked();
            assert_eq!(*actual.borrow(), Some(expected), "{source}");
        }
    }

    #[gpui::test]
    fn numeric_features_reach_nested_gpui_text_and_can_be_cleared(cx: &mut TestAppContext) {
        use std::{cell::RefCell, rc::Rc};
        struct Probe(Rc<RefCell<Option<FontFeatures>>>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let actual = self.0.clone();
                let child = super::apply(
                    div().child(
                        canvas(
                            move |_, window, _| {
                                *actual.borrow_mut() =
                                    Some(window.text_style().font_features.clone());
                            },
                            |_, _, _, _| {},
                        )
                        .size_full(),
                    ),
                    "span",
                    "font-probe",
                    &[],
                    None,
                    cx,
                );
                div()
                    .font_features(FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)])))
                    .size_full()
                    .child(child)
            }
        }
        cx.update(gpui_component::init);
        for (source, expected) in [
            ("", vec![("tnum".into(), 1)]),
            ("span { font-variant-numeric: normal; }", vec![]),
            (
                "body { font-feature-settings: 'tnum' off, 'zero' on; }",
                vec![("tnum".into(), 0), ("zero".into(), 1)],
            ),
        ] {
            cx.update(|cx| {
                cx.set_global(NativeStyles::new(
                    Stylesheet::parse([source]).unwrap(),
                    appearance::Palette::default(),
                ))
            });
            let actual = Rc::new(RefCell::new(None));
            let (_, cx) = cx.add_window_view(|_, _| Probe(actual.clone()));
            cx.run_until_parked();
            assert_eq!(
                actual.borrow().as_ref().unwrap().0.as_ref(),
                &expected,
                "{source}"
            );
        }
    }

    #[::core::prelude::v1::test]
    fn native_css_decorations_cursors_and_alignment_follow_cascade() {
        let palette = appearance::Palette::default();
        let unsupported = NativeStyles::new(
            Stylesheet::parse(["body { text-decoration: overline double red; }"]).unwrap(),
            palette.clone(),
        );
        assert!(unsupported.warnings.is_empty());
        let text = unsupported
            .style(Target::new("body", "native-root", &[]), None, 16.)
            .text
            .unwrap();
        assert_eq!(
            text.overline.unwrap().pattern,
            TextDecorationPattern::Double
        );
        assert_eq!(text.underline.unwrap().thickness, px(0.));
        for (value, expected) in [
            ("solid", TextDecorationPattern::Solid),
            ("dotted", TextDecorationPattern::Dotted),
            ("dashed", TextDecorationPattern::Dashed),
            ("double", TextDecorationPattern::Double),
            ("wavy", TextDecorationPattern::Wavy),
        ] {
            for css in [
                format!(
                    "body {{ text-decoration: underline overline line-through {value} #112233 2px; }}"
                ),
                format!(
                    "body {{ text-decoration-style: {value}; text-decoration-line: underline overline line-through; text-decoration-color: #112233; text-decoration-thickness: 2px; }}"
                ),
            ] {
                let styles =
                    NativeStyles::new(Stylesheet::parse([css.as_str()]).unwrap(), palette.clone());
                assert!(styles.warnings.is_empty());
                let text = styles
                    .style(Target::new("body", "native-root", &[]), None, 16.)
                    .text
                    .unwrap();
                assert_eq!(text.underline.unwrap().pattern, expected);
                assert_eq!(text.overline.unwrap().pattern, expected);
                assert_eq!(text.strikethrough.unwrap().pattern, expected);
                assert_eq!(text.underline.unwrap().thickness, px(2.));
                assert_eq!(
                    text.strikethrough.unwrap().color,
                    Some(rgb(0x112233).into())
                );
            }
        }
        for source in [
            "body { text-decoration-color: #ff0000; text-decoration-thickness: 0.125rem; text-decoration-style: wavy; text-decoration-line: underline line-through; }",
            "body { text-decoration: underline line-through wavy #ff0000 0.125rem; }",
            "body { text-decoration: underline line-through blue 4px; text-decoration-color: #ff0000; text-decoration-thickness: 0.125rem; text-decoration-style: wavy; }",
        ] {
            let styles = NativeStyles::new(Stylesheet::parse([source]).unwrap(), palette.clone());
            assert!(styles.warnings.is_empty(), "{:?}", styles.warnings);
            let style = styles.style(Target::new("body", "native-root", &[]), None, 16.);
            let text = style.text.unwrap();
            let underline = text.underline.unwrap();
            let strike = text.strikethrough.unwrap();
            assert_eq!(underline.thickness, px(2.));
            assert_eq!(strike.thickness, px(2.));
            assert!(underline.wavy);
            assert_eq!(underline.color, Some(rgb(0xff0000).into()));
            assert_eq!(strike.color, underline.color);
        }
        let styles = NativeStyles::new(Stylesheet::parse(["body { text-decoration-color: red; text-decoration: underline line-through wavy 3px; text-decoration: none; cursor: not-allowed; align-items: start; align-self: end; align-content: space-evenly; justify-content: space-evenly; }"]).unwrap(), palette);
        let style = styles.style(Target::new("body", "native-root", &[]), None, 16.);
        assert_eq!(style.mouse_cursor, Some(CursorStyle::OperationNotAllowed));
        assert_eq!(style.align_items, Some(AlignItems::Start));
        assert_eq!(style.align_self, Some(AlignItems::End));
        assert_eq!(style.align_content, Some(AlignContent::SpaceEvenly));
        assert_eq!(style.justify_content, Some(JustifyContent::SpaceEvenly));
        let mut text = TextStyle {
            underline: Some(UnderlineStyle {
                thickness: px(4.),
                ..Default::default()
            }),
            overline: Some(Box::new(UnderlineStyle {
                thickness: px(4.),
                ..Default::default()
            })),
            strikethrough: Some(StrikethroughStyle {
                thickness: px(4.),
                ..Default::default()
            }),
            ..Default::default()
        };
        text.refine(&style.text.unwrap());
        assert_eq!(text.underline.unwrap().thickness, px(0.));
        assert_eq!(text.overline.unwrap().thickness, px(0.));
        assert_eq!(text.strikethrough.unwrap().thickness, px(0.));
        for (value, expected) in [
            ("nwse-resize", CursorStyle::ResizeUpLeftDownRight),
            ("nesw-resize", CursorStyle::ResizeUpRightDownLeft),
            ("grab", CursorStyle::OpenHand),
            ("pointer", CursorStyle::PointingHand),
        ] {
            let mut style = StyleRefinement::default();
            super::apply_property(
                &mut style,
                "cursor",
                value,
                &appearance::Palette::default(),
                16.,
            );
            assert_eq!(style.mouse_cursor, Some(expected));
        }
    }

    #[::core::prelude::v1::test]
    fn native_css_layout_typography_borders_and_layered_shadows() {
        let palette = appearance::Palette::default();
        let sheet = Stylesheet::parse(["body { position: absolute; top: 12px; right: 10%; aspect-ratio: 16 / 9; flex: 1 2 20%; flex-wrap: wrap; row-gap: 8px; column-gap: 1rem; border: rgb(255 0 0 / 50%) 2px dashed; border-width: 1px 2px 3px 4px; line-height: 1.5; white-space: nowrap; text-overflow: ellipsis; box-shadow: 0 4px 12px rgb(0 0 0 / 20%), -1px 0 2px 1px #ff0000; }"]).unwrap();
        let styles = NativeStyles::new(sheet, palette.clone());
        assert!(styles.warnings.is_empty(), "{:?}", styles.warnings);
        let style = styles.style(Target::new("body", "native-root", &[]), None, 16.);
        assert_eq!(style.position, Some(Position::Absolute));
        assert_eq!(style.inset.top, Some(px(12.).into()));
        assert_eq!(style.inset.right, Some(relative(0.1).into()));
        assert_eq!(style.aspect_ratio, Some(16. / 9.));
        assert_eq!(style.flex_grow, Some(1.));
        assert_eq!(style.flex_shrink, Some(2.));
        assert_eq!(style.flex_basis, Some(relative(0.2).into()));
        assert_eq!(style.flex_wrap, Some(FlexWrap::Wrap));
        assert_eq!(style.gap.height, Some(px(8.).into()));
        assert_eq!(style.gap.width, Some(px(16.).into()));
        assert_eq!(style.border_style, Some(BorderStyle::Dashed));
        assert_eq!(style.border_widths.bottom, Some(px(3.).into()));
        assert_eq!(style.border_color.unwrap().a, 0.5);
        let text = style.text.unwrap();
        assert_eq!(text.white_space, Some(WhiteSpace::Nowrap));
        assert_eq!(text.line_height, Some(relative(1.5)));
        assert_eq!(text.text_overflow, Some(TextOverflow::Truncate("…".into())));
        let shadows = style.box_shadow.unwrap();
        assert_eq!(shadows.len(), 2);
        assert_eq!(shadows[0].offset, point(px(0.), px(4.)));
        assert_eq!(shadows[0].blur_radius, px(12.));
        assert_eq!(shadows[0].color.a, 0.2);
        assert_eq!(shadows[1].spread_radius, px(1.));
        assert!(super::shadows("0 0 -1px red", &palette, 16.).is_none());
        assert!(super::shadows("0 0 rgb(2 3 4),", &palette, 16.).is_none());
    }
    #[gpui::test]
    fn css_variables_preserve_original_computed_inheritance_and_resets(cx: &mut TestAppContext) {
        struct Probe;
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
            }
        }
        let (_, cx) = cx.add_window_view(|_, _| Probe);
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/css-variable-cascades-reference.json"
        ))
        .unwrap();
        let mut failures = vec![];
        for case in cases.as_array().unwrap() {
            let compute = |source: &str| {
                NativeStyles::new(
                    Stylesheet::parse([source]).unwrap(),
                    appearance::Palette::default(),
                )
                .style(Target::new("span", "probe", &["probe"]), None, 16.)
            };
            let expected_source = format!(
                "body {{{}:{}}} span {{{}:{}}}",
                case["property"].as_str().unwrap(),
                case["parent"].as_str().unwrap(),
                case["property"].as_str().unwrap(),
                case["expected"].as_str().unwrap()
            );
            let mut actual = compute(case["source"].as_str().unwrap());
            let mut expected = compute(&expected_source);
            let images_match = match (&actual.background, &expected.background) {
                (Some(Fill::Image(a)), Some(Fill::Image(b))) => cx.update(|window, _| {
                    let bounds = size(px(32.), px(32.));
                    a.color == b.color
                        && a.inherit_background_color == b.inherit_background_color
                        && a.image(bounds, window).as_bytes(0)
                            == b.image(bounds, window).as_bytes(0)
                }),
                _ => true,
            };
            if images_match
                && matches!(actual.background, Some(Fill::Image(_)))
                && matches!(expected.background, Some(Fill::Image(_)))
            {
                actual.background = None;
                expected.background = None;
            }
            if !images_match || actual != expected {
                failures.push(format!(
                    "{}: {} should compute {}",
                    case["property"], case["value"], case["expected"]
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn flex_shorthand_matches_original_basis_order_and_invalid_cascades() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/css-flex-reference.json")).unwrap();
        let mut failures = vec![];
        for case in cases.as_array().unwrap() {
            let actual = NativeStyles::new(
                Stylesheet::parse([case["source"].as_str().unwrap()]).unwrap(),
                appearance::Palette::default(),
            )
            .style(Target::new("div", "probe", &["probe"]), None, 16.);
            let expected = (
                Some(case["grow"].as_f64().unwrap() as f32),
                Some(case["shrink"].as_f64().unwrap() as f32),
                layout_length(case["basis"].as_str().unwrap(), 16.),
            );
            let actual = (actual.flex_grow, actual.flex_shrink, actual.flex_basis);
            if actual != expected {
                failures.push(format!("{}: {actual:?} != {expected:?}", case["value"]));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn invalid_css_values_preserve_earlier_valid_declarations() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/css-invalid-values-reference.json"))
                .unwrap();
        let mut failures = vec![];
        for case in cases.as_array().unwrap() {
            let valid = format!(
                ".probe {{{}:{}}}",
                case["property"].as_str().unwrap(),
                case["valid"].as_str().unwrap()
            );
            let compute = |source: &str| {
                NativeStyles::new(
                    Stylesheet::parse([".probe {text-decoration:underline}", source]).unwrap(),
                    appearance::Palette::default(),
                )
                .style(Target::new("div", "probe", &["probe"]), None, 16.)
            };
            let expected = compute(&valid);
            let actual = compute(case["source"].as_str().unwrap());
            if actual != expected {
                failures.push(format!("{}: {}", case["property"], case["invalid"]));
            }
        }
        assert!(
            failures.is_empty(),
            "Invalid CSS changed styles: {failures:?}"
        );
    }

    #[test]
    fn foreground_filters_follow_original_css_cascades() {
        use feishin_core::styles::BackdropFilter;
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/foreground-filters-reference.json"))
                .unwrap();
        for platform in reference["platform_metrics"].as_object().unwrap().values() {
            for case in platform["cascades"].as_array().unwrap() {
                let native = NativeStyles::new(
                    Stylesheet::parse([case["source"].as_str().unwrap()]).unwrap(),
                    appearance::Palette::default(),
                );
                assert!(
                    !native
                        .warnings
                        .iter()
                        .any(|warning| warning.contains("filter")),
                    "{case}: {:?}",
                    native.warnings
                );
                let style = native.style(Target::new("span", "probe", &[]), None, 16.);
                let value = case["filter"].as_str().unwrap();
                let expected = match feishin_core::styles::backdrop_filter(value, 16.).unwrap() {
                    BackdropFilter::Blur(_) if value == "none" => vec![],
                    BackdropFilter::Blur(sigma) => vec![gpui::BackdropFilterOperation::Blur(sigma)],
                    BackdropFilter::BlurSaturate(sigma, saturation) => vec![
                        gpui::BackdropFilterOperation::Blur(sigma),
                        gpui::BackdropFilterOperation::Saturate(saturation),
                    ],
                    BackdropFilter::Chain(operations) => operations
                        .into_iter()
                        .map(native_filter_operation)
                        .collect(),
                    BackdropFilter::Inherit => panic!("Computed inherit: {case}"),
                };
                let actual = style.foreground_filters.unwrap_or_default();
                assert_eq!(actual.len(), expected.len(), "{case}");
                for (actual, expected) in actual.iter().zip(&expected) {
                    assert_eq!(
                        std::mem::discriminant(actual),
                        std::mem::discriminant(expected),
                        "{case}"
                    );
                    match (actual, expected) {
                        (
                            gpui::BackdropFilterOperation::DropShadow {
                                offset: a,
                                sigma: sa,
                                color: ca,
                            },
                            gpui::BackdropFilterOperation::DropShadow {
                                offset: b,
                                sigma: sb,
                                color: cb,
                            },
                        ) => {
                            assert_eq!(a, b, "{case}");
                            assert_eq!(sa, sb, "{case}");
                            let ca = ca.unwrap_or([0., 0., 0., 1.]);
                            let cb = cb.unwrap_or([0., 0., 0., 1.]);
                            assert!(
                                ca.into_iter()
                                    .zip(cb)
                                    .all(|(a, b)| (a - b).abs() <= 1. / 255.),
                                "{case}: {ca:?} != {cb:?}"
                            );
                        }
                        (
                            gpui::BackdropFilterOperation::HueRotate(a),
                            gpui::BackdropFilterOperation::HueRotate(b),
                        ) => assert!((a - b).abs() < 0.00001, "{case}"),
                        _ => assert_eq!(actual, expected, "{case}"),
                    }
                }
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn foreground_pixels_match_original_filter_layers(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        struct Probe(serde_json::Value, Option<std::sync::Arc<gpui::RenderImage>>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let case = &self.0;
                let root = div().relative().w(px(256.)).h(px(256.)).child(
                    canvas(
                        |_, _, _| (),
                        |_, _, window, _| {
                            for row in 0..8 {
                                for column in 0..8 {
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(px(column as f32 * 32.), px(row as f32 * 32.)),
                                            size(px(32.), px(32.)),
                                        ),
                                        rgb(if (row + column) % 2 == 0 {
                                            0x506580
                                        } else {
                                            0x2a3550
                                        }),
                                    ));
                                }
                            }
                        },
                    )
                    .absolute()
                    .size_full(),
                );
                let mut first: Hsla = rgb(0xf0a050).into();
                first.a = case["sourceOpacity"].as_f64().unwrap_or(1.) as f32;
                let mut layer = super::apply(
                    div()
                        .absolute()
                        .left(px(40.))
                        .top(px(36.))
                        .w(px(168.))
                        .h(px(156.)),
                    "div",
                    "filter",
                    &["probe"],
                    None,
                    cx,
                )
                .paint_scale(case["scale"].as_f64().unwrap_or(1.) as f32)
                .when(
                    case["sourceClipped"].as_bool() == Some(true)
                        && (!case["picture"].as_bool().unwrap_or(false)
                            || case["sourceRadius"].as_f64().unwrap_or(0.) > 0.),
                    |div| div.overflow_hidden(),
                )
                .rounded(px(case["sourceRadius"].as_f64().unwrap_or(0.) as f32));
                if let Some(image) = &self.1 {
                    layer = layer.child(
                        gpui::img(image.clone())
                            .absolute()
                            .inset_0()
                            .size_full()
                            .object_fit(gpui::ObjectFit::Cover),
                    );
                } else {
                    layer = layer
                        .child(
                            div()
                                .absolute()
                                .left(px(-12.))
                                .top(px(20.))
                                .w(px(100.))
                                .h(px(88.))
                                .bg(first),
                        )
                        .child(
                            div()
                                .absolute()
                                .left(px(60.))
                                .top(px(60.))
                                .w(px(96.))
                                .h(px(100.))
                                .bg(rgba(0x41658abf)),
                        );
                }
                if case["nested"].is_string() {
                    layer = layer.child(super::apply(
                        div()
                            .absolute()
                            .left(px(42.))
                            .top(px(16.))
                            .w(px(52.))
                            .h(px(56.))
                            .bg(rgba(0x20f04080)),
                        "i",
                        "nested",
                        &["nested"],
                        Some(Target::new("div", "filter", &["probe"])),
                        cx,
                    ));
                }
                let clipped = case["clipped"].as_bool().unwrap_or(false);
                root.child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .top(px(0.))
                        .w(px(if clipped { 140. } else { 256. }))
                        .h(px(if clipped { 140. } else { 256. }))
                        .when(clipped, |div| div.overflow_hidden())
                        .rounded(px(case["parentRadius"].as_f64().unwrap_or(0.) as f32))
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .opacity(case["parentOpacity"].as_f64().unwrap_or(1.) as f32)
                                .text_color({
                                    let [r, g, b, a] = appearance::color(
                                        case["parentColor"].as_str().unwrap_or("black"),
                                    )
                                    .unwrap();
                                    Rgba { r, g, b, a }
                                })
                                .child(layer),
                        ),
                )
            }
        }
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/foreground-filters-reference.json"))
                .unwrap();
        let platform = if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        };
        let mut failures = vec![];
        for case in reference["platform_metrics"][platform]["cases"]
            .as_array()
            .unwrap()
        {
            let filter = case["filter"].as_str().unwrap();
            let css = format!(
                ".probe {{color:{};background:{};filter:{filter};opacity:{}}}.nested{{filter:{}}}",
                case["color"]
                    .as_str()
                    .unwrap_or(if case["parentColor"].is_string() {
                        "inherit"
                    } else {
                        "black"
                    }),
                case["layerBackground"].as_str().unwrap_or("#12345680"),
                case["opacity"].as_f64().unwrap_or(1.),
                case["nested"].as_str().unwrap_or("none")
            );
            cx.update(|cx| {
                cx.set_global(NativeStyles::new(
                    Stylesheet::parse([css.as_str()]).unwrap(),
                    appearance::Palette::default(),
                ))
            });
            let source = case["sourceFile"].as_str().map(|file| {
                let mut image = image::open(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets")
                        .join(file),
                )
                .unwrap()
                .into_rgba8();
                for pixel in image.pixels_mut() {
                    pixel.0.swap(0, 2);
                }
                std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(image)]))
            });
            let (_, window_cx) = cx.add_window_view(|_, _| Probe(case.clone(), source));
            window_cx.run_until_parked();
            window_cx.update(|window, _| {
                let pixels = window
                    .rasterized_scene_for_test(size(px(256.), px(256.)))
                    .unwrap();
                let expected = image::open(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets")
                        .join(case["file"].as_str().unwrap()),
                )
                .unwrap()
                .into_rgba8();
                assert_eq!(expected.dimensions(), (512, 512));
                let mut expected = expected.into_raw();
                for pixel in expected.chunks_exact_mut(4) {
                    for channel in 0..3 {
                        pixel[channel] =
                            (f32::from(pixel[channel]) * f32::from(pixel[3]) / 255.).round() as u8;
                    }
                }
                if let Some(directory) = std::env::var_os("FOREGROUND_ACTUAL_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    image::save_buffer(
                        std::path::Path::new(&directory).join(case["file"].as_str().unwrap()),
                        &pixels,
                        512,
                        512,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
                let square: u64 = pixels
                    .iter()
                    .zip(&expected)
                    .map(|(a, b)| u64::from(a.abs_diff(*b)).pow(2))
                    .sum();
                let maximum = pixels
                    .iter()
                    .zip(&expected)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                let rms = (square as f64 / pixels.len() as f64).sqrt();
                eprintln!("Foreground {}: rms={rms:.4} max={maximum}", case["name"]);
                let has_blur = (filter.contains("blur(") && filter != "blur(0)") || filter.contains("drop-shadow(");
                let sharp_picture = !has_blur && case["picture"].as_bool() == Some(true);
                let rounded = case["sourceRadius"].as_f64().unwrap_or(0.) > 0.
                    || case["parentRadius"].as_f64().unwrap_or(0.) > 0.;
                // Chromium's Linux software compositor clips rounded edges per
                // draw; native groups composite them once. Keep that allowance
                // restricted to the one-pixel perimeter and retain strict interiors.
                let rounded_edge_limit = if cfg!(target_os = "linux") { 64 } else { 40 };
                let tolerance = if has_blur { 1.05 }
                    else if rounded && cfg!(target_os = "linux") { 0.65 } else { 0.55 };
                let perimeter_limit = if rounded { rounded_edge_limit }
                    else if case["picture"].as_bool() == Some(true) && !filter.contains("blur(") { 20 } else { 16 };
                if rms > tolerance || maximum > perimeter_limit {
                    failures.push(format!("{}: rms={rms}, max={maximum}", case["name"]));
                }
                if rounded {
                    let scale = case["scale"].as_f64().unwrap_or(1.);
                    let source_radius = case["sourceRadius"].as_f64().unwrap_or(0.) * scale * 2.;
                    let parent_radius = case["parentRadius"].as_f64().unwrap_or(0.) * 2.;
                    let on_edge = |x: f64, y: f64, bounds: [f64; 4], radius: f64| {
                        if radius == 0. { return false; }
                        let hx = (bounds[2] - bounds[0]) * 0.5;
                        let hy = (bounds[3] - bounds[1]) * 0.5;
                        let qx = (x - (bounds[0] + hx)).abs() - hx + radius;
                        let qy = (y - (bounds[1] + hy)).abs() - hy + radius;
                        (qx.max(0.).hypot(qy.max(0.)) + qx.max(qy).min(0.) - radius).abs() <= 1.
                    };
                    for y in 0..512 {
                        for x in 0..512 {
                            let offset = (y * 512 + x) * 4;
                            let error = pixels[offset..offset+4].iter().zip(&expected[offset..offset+4]).map(|(a,b)| a.abs_diff(*b)).max().unwrap();
                            if error <= 16 { continue; }
                            let x = x as f64 + 0.5;
                            let y = y as f64 + 0.5;
                            assert!(on_edge(x, y, [0.,0.,280.,280.], parent_radius)
                                || on_edge(x, y, [(124.-84.*scale)*2.,(114.-78.*scale)*2.,(124.+84.*scale)*2.,(114.+78.*scale)*2.], source_radius),
                                "Rounded clipping changed a pixel away from its antialiased edge: {case} at {x},{y}, error {error}");
                        }
                    }
                }
                if sharp_picture {
                    let scale = case["scale"].as_f64().unwrap_or(1.);
                    let edges = [(124. - 84. * scale) * 2., (124. + 84. * scale) * 2., (114. - 78. * scale) * 2., (114. + 78. * scale) * 2.];
                    for y in 0..512 {
                        for x in 0..512 {
                            let offset = (y * 512 + x) * 4;
                            let error = pixels[offset..offset+4].iter().zip(&expected[offset..offset+4]).map(|(a,b)| a.abs_diff(*b)).max().unwrap();
                            let x = x as f64 + 0.5;
                            let y = y as f64 + 0.5;
                            let radius = case["sourceRadius"].as_f64().unwrap_or(0.) * scale * 2.;
                            let corner_x = if x < edges[0] + radius { Some(edges[0] + radius) }
                                else if x > edges[1] - radius { Some(edges[1] - radius) } else { None };
                            let corner_y = if y < edges[2] + radius { Some(edges[2] + radius) }
                                else if y > edges[3] - radius { Some(edges[3] - radius) } else { None };
                            let perimeter = if let (Some(cx), Some(cy)) = (corner_x, corner_y) {
                                ((x - cx).hypot(y - cy) - radius).abs() <= 1.
                            } else {
                                (x - edges[0]).abs().min((x - edges[1]).abs()).min((y - edges[2]).abs()).min((y - edges[3]).abs()) <= 1.
                            };
                            let seam = (x - 248.).abs() <= 12. * scale;
                            let bound = if perimeter && radius > 0. { rounded_edge_limit }
                                else if perimeter { 20 } else if seam { 10 } else { 2 };
                            assert!(error <= bound, "Artwork sample outside its edge/seam allowance: {case} at {x},{y}, error {error}");
                        }
                    }
                }
                // Color-only filters must leave the patterned backdrop untouched
                // and agree inside the overlapping translucent source.
                if !has_blur && case["picture"].as_bool() == Some(true) {
                    for (x, y) in [(80, 80), (150, 100)] {
                        let offset = (y * 2 * 512 + x * 2) * 4;
                        assert!(
                            pixels[offset..offset + 4]
                                .iter()
                                .zip(&expected[offset..offset + 4])
                                .all(|(a, b)| a.abs_diff(*b) <= 1),
                            "Artwork interior differs: {case}"
                        );
                    }
                }
                if !has_blur && case["nested"].is_null() && case["scale"].is_null() {
                    for (x, y) in [(16, 16), (220, 220), (120, 110), (60, 80)] {
                        let offset = (y * 2 * 512 + x * 2) * 4;
                        assert!(
                            pixels[offset..offset + 4]
                                .iter()
                                .zip(&expected[offset..offset + 4])
                                .all(|(a, b)| a.abs_diff(*b) <= 1),
                            "{case} at {x},{y}: {:?} != {:?}",
                            &pixels[offset..offset + 4],
                            &expected[offset..offset + 4]
                        );
                    }
                }
            });
        }
        assert!(
            failures.is_empty(),
            "Foreground pixel differences: {failures:?}"
        );
    }

    #[test]
    fn backdrop_filters_follow_original_css_cascades() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/backdrop-filters-reference.json"))
                .unwrap();
        for platform in reference["platform_metrics"].as_object().unwrap().values() {
            for case in platform["cascades"].as_array().unwrap() {
                let native = NativeStyles::new(
                    Stylesheet::parse([case["source"].as_str().unwrap()]).unwrap(),
                    appearance::Palette::default(),
                );
                assert!(
                    !native
                        .warnings
                        .iter()
                        .any(|warning| warning.contains("backdrop-filter"))
                );
                let style = native.style(Target::new("span", "probe", &[]), None, 16.);
                let expected = case["filter"].as_str().unwrap();
                let (sigma, saturation, operations) =
                    match feishin_core::styles::backdrop_filter(expected, 16.).unwrap() {
                        feishin_core::styles::BackdropFilter::Blur(sigma) => (sigma, 1., vec![]),
                        feishin_core::styles::BackdropFilter::BlurSaturate(sigma, saturation) => {
                            (sigma, saturation, vec![])
                        }
                        feishin_core::styles::BackdropFilter::Chain(operations) => (
                            0.,
                            1.,
                            operations
                                .into_iter()
                                .map(native_filter_operation)
                                .collect(),
                        ),
                        feishin_core::styles::BackdropFilter::Inherit => {
                            panic!("Computed value retained inherit: {case}")
                        }
                    };
                assert_eq!(style.backdrop_blur.unwrap_or(px(0.)), px(sigma), "{case}");
                assert_eq!(
                    style.backdrop_saturation.unwrap_or(1.),
                    saturation,
                    "{case}"
                );
                let actual = style.backdrop_filters.unwrap_or_default();
                assert_eq!(actual.len(), operations.len(), "{case}");
                for (actual, expected) in actual.iter().zip(operations.iter()) {
                    match (actual, expected) {
                        (
                            BackdropFilterOperation::HueRotate(a),
                            BackdropFilterOperation::HueRotate(b),
                        ) => assert!((a - b).abs() < 0.00001, "{case}"),
                        _ => assert_eq!(actual, expected, "{case}"),
                    }
                }
            }
        }
    }

    #[gpui::test]
    fn live_font_reload_refreshes_multiline_input_wrapping(cx: &mut TestAppContext) {
        use gpui_component::{Root, input::InputState};
        use std::{borrow::Cow, rc::Rc};
        let system = cx.use_platform_text_system();
        system
            .add_fonts(vec![Cow::Borrowed(include_bytes!(
                "../assets/font-reload-1.ttf"
            ))])
            .unwrap();
        cx.update(gpui_component::init);
        struct Probe(Entity<InputState>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().w(px(90.)).h(px(500.)).child(
                    Input::new(&self.0)
                        .bordered(false)
                        .p_0()
                        .h(px(500.))
                        .font_family("Feishin Reload Probe")
                        .text_size(px(32.))
                        .line_height(relative(1.5)),
                )
            }
        }
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| InputState::new(window, cx).multi_line(true).rows(10));
            input.update(cx, |input, cx| input.set_value("HHH HHH HHH", window, cx));
            captured.replace(Some(input.clone()));
            let probe = cx.new(|_| Probe(input));
            Root::new(probe, window, cx)
        });
        let input = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let rows = |cx: &mut VisualTestContext| {
            cx.update(|window, _| {
                window
                    .painted_glyphs_for_test()
                    .into_iter()
                    .map(|(bounds, _, _)| (bounds.center().y / px(48.)).floor() as i32)
                    .collect::<std::collections::BTreeSet<_>>()
            })
        };
        let first = rows(cx);
        assert_eq!(first.len(), 3, "Original font rows: {first:?}");
        system
            .add_fonts(vec![Cow::Borrowed(include_bytes!(
                "../assets/font-reload-2.ttf"
            ))])
            .unwrap();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let second = rows(cx);
        assert!(
            second.len() > first.len(),
            "Input kept stale font wrapping: {first:?} -> {second:?}"
        );
        cx.update(|_, cx| assert_eq!(input.read(cx).value(), "HHH HHH HHH"));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn backdrop_pixels_match_original_glassy_theme(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        struct Probe(serde_json::Value);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let case = &self.0;
                let x = case["x"].as_f64().unwrap() as f32;
                let y = case["y"].as_f64().unwrap() as f32;
                let class = case["className"].as_str().unwrap_or("");
                let source_opacity = case["sourceOpacity"].as_f64().unwrap_or(1.) as f32;
                let fine = case["fine"].as_bool().unwrap_or(false);
                let mut root = div().relative().w(px(256.)).h(px(256.)).child(
                    canvas(
                        |_, _, _| (),
                        move |_, _, window, _| {
                            if fine {
                                for x in 0..512 {
                                    let color = rgb(if ((x + 1) / 2) % 2 == 0 {
                                        0xf0a050
                                    } else {
                                        0x41658a
                                    });
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(px(x as f32 / 2.), px(0.)),
                                            size(px(0.5), px(256.)),
                                        ),
                                        color,
                                    ));
                                }
                                return;
                            }
                            for row in 0..8 {
                                for column in 0..8 {
                                    let mut color: Hsla = rgb(if (row + column) % 2 == 0 {
                                        0xf0a050
                                    } else {
                                        0x41658a
                                    })
                                    .into();
                                    color.a *= source_opacity;
                                    window.paint_quad(fill(
                                        Bounds::new(
                                            point(px(column as f32 * 32.), px(row as f32 * 32.)),
                                            size(px(32.), px(32.)),
                                        ),
                                        color,
                                    ));
                                }
                            }
                        },
                    )
                    .absolute()
                    .size_full(),
                );
                let surface = super::apply(
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(168.))
                        .h(px(156.)),
                    "div",
                    "filter",
                    &["probe", class],
                    None,
                    cx,
                )
                .child(
                    div()
                        .absolute()
                        .left(px(24.))
                        .top(px(if case["overlap"].as_bool() == Some(true) {
                            88.
                        } else {
                            28.
                        }))
                        .w(px(72.))
                        .h(px(4.))
                        .bg(rgb(0xffffff)),
                );
                let surface = if let Some(opacity) = case["parentOpacity"].as_f64() {
                    div()
                        .absolute()
                        .size_full()
                        .opacity(opacity as f32)
                        .child(surface)
                        .into_any_element()
                } else {
                    surface.into_any_element()
                };
                root = if case["clipped"].as_bool() == Some(true) {
                    root.child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(0.))
                            .w(px(140.))
                            .h(px(140.))
                            .overflow_hidden()
                            .child(surface),
                    )
                } else {
                    root.child(surface)
                };
                if case["overlap"].as_bool() == Some(true) {
                    root = root.child(
                        div()
                            .absolute()
                            .left(px(104.))
                            .top(px(104.))
                            .w(px(104.))
                            .h(px(112.))
                            .rounded(px(18.))
                            .backdrop_blur(px(5.))
                            .bg(rgba(0x00000033))
                            .child(
                                div()
                                    .absolute()
                                    .left(px(12.))
                                    .top(px(24.))
                                    .w(px(64.))
                                    .h(px(4.))
                                    .bg(rgb(0x20f040)),
                            ),
                    );
                }
                root
            }
        }
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/backdrop-filters-reference.json"))
                .unwrap();
        let platform = if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        };
        let mut failures = vec![];
        for case in reference["platform_metrics"][platform]["cases"]
            .as_array()
            .unwrap()
        {
            let filter = case["filter"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("blur({}px)", case["sigma"]));
            let css = format!(
                ".probe {{background:{};border-radius:{}px;backdrop-filter:{};opacity:{}}}",
                case["background"].as_str().unwrap(),
                case["radius"],
                filter,
                case["opacity"].as_f64().unwrap_or(1.)
            );
            let native = NativeStyles::new(
                Stylesheet::parse([include_str!("../assets/styles/glassy-dark.css"), &css])
                    .unwrap(),
                appearance::Palette::default(),
            );
            cx.update(|cx| cx.set_global(native));
            let (_, window_cx) = cx.add_window_view(|_, _| Probe(case.clone()));
            window_cx.run_until_parked();
            window_cx.update(|window, _| {
                let blurs = window.painted_backdrops_for_test();
                let sigma = case["sigma"].as_f64().unwrap() as f32;
                let filtered = sigma > 0.
                    || case["saturation"].as_f64().unwrap_or(1.) != 1.
                    || (case["ordered"].as_bool() == Some(true)
                        && case["groupOnly"].as_bool() != Some(true));
                assert_eq!(
                    blurs.len(),
                    usize::from(filtered) + usize::from(case["overlap"].as_bool() == Some(true)),
                    "{case}"
                );
                if filtered {
                    assert_eq!(blurs[0].0.size, size(px(168.), px(156.)));
                    assert_eq!(
                        blurs[0].1.top_left,
                        px(case["radius"].as_f64().unwrap() as f32)
                    );
                    assert_eq!(blurs[0].3, px(sigma));
                    assert_eq!(blurs[0].4, case["opacity"].as_f64().unwrap_or(1.) as f32);
                }
                let pixels = window
                    .rasterized_scene_for_test(size(px(256.), px(256.)))
                    .unwrap();
                let expected = image::open(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets")
                        .join(case["file"].as_str().unwrap()),
                )
                .unwrap()
                .into_rgba8();
                assert_eq!(expected.dimensions(), (512, 512));
                let mut expected_pixels = expected.into_raw();
                for pixel in expected_pixels.chunks_exact_mut(4) {
                    for channel in 0..3 {
                        pixel[channel] =
                            (f32::from(pixel[channel]) * f32::from(pixel[3]) / 255.).round() as u8;
                    }
                }
                assert_eq!(pixels.len(), expected_pixels.as_slice().len());
                if let Some(directory) = std::env::var_os("BACKDROP_ACTUAL_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    image::save_buffer(
                        std::path::Path::new(&directory).join(case["file"].as_str().unwrap()),
                        &pixels,
                        512,
                        512,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
                let mut square = 0u64;
                let mut largest = 0u8;
                for (actual, expected) in pixels.iter().zip(expected_pixels.as_slice()) {
                    let error = actual.abs_diff(*expected);
                    square += u64::from(error) * u64::from(error);
                    largest = largest.max(error);
                }
                let rms = (square as f64 / pixels.len() as f64).sqrt();
                eprintln!("Backdrop {}: rms={rms:.4} max={largest}", case["name"]);
                // Bound the whole image tightly, while allowing single pixel
                // perimeter differences of SDF and Chromium area AA.
                // Saturation amplifies the existing Gaussian/software-blur
                // approximation error. Color-only filters retain the 1/255 bound.
                let gain = if sigma > 0. {
                    use feishin_core::styles::{BackdropFilter, FilterOperation};
                    match feishin_core::styles::backdrop_filter(&filter, 16.).unwrap() {
                        BackdropFilter::BlurSaturate(_, saturation) => {
                            f64::from(saturation).max(1.)
                        }
                        BackdropFilter::Chain(operations) => {
                            operations
                                .into_iter()
                                .fold(1., |gain, operation| match operation {
                                    FilterOperation::Saturate(n)
                                    | FilterOperation::Brightness(n)
                                    | FilterOperation::Contrast(n) => gain * f64::from(n).max(1.),
                                    _ => gain,
                                })
                        }
                        _ => 1.,
                    }
                } else {
                    1.
                };
                // High-contrast color effects reveal the same single-pixel SDF /
                // Chromium path-AA difference more strongly. Keep legacy bounds,
                // and independently bound interior color output below.
                let perimeter_limit = if case["ordered"].as_bool() == Some(true) {
                    72
                } else {
                    48
                };
                if rms > gain || largest > perimeter_limit {
                    failures.push(format!("{}: rms={rms}, max={largest}", case["name"]));
                }
                if sigma == 0. && case["ordered"].as_bool() == Some(true) {
                    let offset = (220 * 512 + 220) * 4;
                    assert!(
                        pixels[offset..offset + 4]
                            .iter()
                            .zip(&expected_pixels[offset..offset + 4])
                            .all(|(a, b)| a.abs_diff(*b) <= 1),
                        "Interior color differs: {case}"
                    );
                }
                for (x, y) in [(250, 250), (20, 250), (224, 96)] {
                    let offset = (y * 2 * 512 + x * 2) * 4;
                    assert!(
                        pixels[offset..offset + 4]
                            .iter()
                            .zip(&expected_pixels.as_slice()[offset..offset + 4])
                            .all(|(a, b)| a.abs_diff(*b) <= 1),
                        "Outside backdrop changed: {case}"
                    );
                }
                let sharp_x = ((case["x"].as_f64().unwrap() + 60.) * 2.) as usize;
                let sharp_y = ((case["y"].as_f64().unwrap()
                    + if case["overlap"].as_bool() == Some(true) {
                        90.
                    } else {
                        30.
                    })
                    * 2.) as usize;
                let offset = (sharp_y * 512 + sharp_x) * 4;
                if !pixels[offset..offset + 4]
                    .iter()
                    .zip(&expected_pixels[offset..offset + 4])
                    .all(|(a, b)| a.abs_diff(*b) <= 2)
                {
                    failures.push(format!("Own controls differ: {}", case["name"]));
                }
            });
        }
        assert!(
            failures.is_empty(),
            "Backdrop pixel differences: {failures:?}"
        );
    }
}
#[::core::prelude::v1::test]
fn css_font_family_keeps_ordered_quoted_and_escaped_fallbacks() {
    let styles = NativeStyles::new(Stylesheet::parse([r#"body { font-family: 'Name, comma', Noto\20 Sans\20 JP, 'Noto Sans Hebrew'; } span { font-family: Inter; }"#]).unwrap(), appearance::builtins()["defaultTheme"].clone());
    let root = Target::new("body", "", &[]);
    let style = styles.style(root.clone(), None, 16.);
    let text = style.text.unwrap();
    assert_eq!(text.font_family.unwrap().as_ref(), "Name, comma");
    assert_eq!(
        text.font_fallbacks.unwrap().fallback_list(),
        &["Noto Sans JP", "Noto Sans Hebrew"]
    );
    let child = styles.style(Target::new("span", "", &[]), Some(root), 16.);
    assert!(
        child
            .text
            .unwrap()
            .font_fallbacks
            .unwrap()
            .fallback_list()
            .is_empty()
    );
}

#[cfg(test)]
#[path = "button_motion_tests.rs"]
mod motion_tests;

#[cfg(test)]
#[path = "button_layout_tests.rs"]
mod button_layout_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "button_loading_tests.rs"]
mod button_loading_tests;
