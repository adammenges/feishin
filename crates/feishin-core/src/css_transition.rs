//! CSS transition lists used by native controls. Declarations arrive in cascade order.
use cssparser::{Parser, ParserInput, Token};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Bezier([f32; 4]),
    Steps { count: u32, start: bool, end: bool },
}
impl Default for Easing {
    fn default() -> Self {
        Self::Bezier([0.25, 0.1, 0.25, 1.])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timing {
    pub duration: f32,
    pub delay: f32,
    pub easing: Easing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transitions {
    properties: Vec<String>,
    durations: Vec<f32>,
    delays: Vec<f32>,
    easings: Vec<Easing>,
}
impl Default for Transitions {
    fn default() -> Self {
        Self {
            properties: vec!["all".into()],
            durations: vec![0.],
            delays: vec![0.],
            easings: vec![Easing::default()],
        }
    }
}
impl Transitions {
    pub fn for_property(&self, name: &str) -> Timing {
        self.properties
            .iter()
            .enumerate()
            .rev()
            .find(|(_, property)| {
                property.as_str() == "all"
                    || property.as_str() == name
                    || (property.as_str() == "background" && name == "background-color")
                    || (matches!(property.as_str(), "border" | "border-color")
                        && name.starts_with("border-")
                        && name.ends_with("-color"))
            })
            .map(|(index, _)| Timing {
                duration: self.durations[index % self.durations.len()],
                delay: self.delays[index % self.delays.len()],
                easing: self.easings[index % self.easings.len()],
            })
            .unwrap_or_default()
    }

    /// Invalid declarations leave the previous cascade value intact.
    pub fn apply(&mut self, name: &str, value: &str, parent: &Self) -> bool {
        if !matches!(
            name,
            "transition"
                | "transition-property"
                | "transition-duration"
                | "transition-delay"
                | "transition-timing-function"
        ) {
            return false;
        }
        let value = value.trim();
        let initial = Self::default();
        let reset = match value.to_ascii_lowercase().as_str() {
            "inherit" => Some(parent),
            "initial" | "unset" | "revert" | "revert-layer" => Some(&initial),
            _ => None,
        };
        if let Some(reset) = reset {
            match name {
                "transition" => *self = reset.clone(),
                "transition-property" => self.properties.clone_from(&reset.properties),
                "transition-duration" => self.durations.clone_from(&reset.durations),
                "transition-delay" => self.delays.clone_from(&reset.delays),
                _ => self.easings.clone_from(&reset.easings),
            }
            return true;
        }
        let Some(parts) = comma_values(value) else {
            return false;
        };
        match name {
            "transition-property" => {
                let Some(properties) = parts
                    .iter()
                    .map(|part| property(part))
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                if properties.len() > 1 && properties.iter().any(|p| p == "none") {
                    return false;
                }
                self.properties = properties;
            }
            "transition-duration" | "transition-delay" => {
                let Some(values) = parts
                    .iter()
                    .map(|part| seconds(part))
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                if name == "transition-duration" {
                    if values.iter().any(|value| *value < 0.) {
                        return false;
                    }
                    self.durations = values;
                } else {
                    self.delays = values;
                }
            }
            "transition-timing-function" => {
                let Some(values) = parts
                    .iter()
                    .map(|part| easing(part))
                    .collect::<Option<Vec<_>>>()
                else {
                    return false;
                };
                self.easings = values;
            }
            _ => {
                let mut parsed = Self {
                    properties: vec![],
                    durations: vec![],
                    delays: vec![],
                    easings: vec![],
                };
                for part in &parts {
                    let Some(tokens) = crate::styles::component_values(part) else {
                        return false;
                    };
                    let (mut property_name, mut duration, mut delay, mut curve) =
                        (None, None, None, None);
                    for token in tokens {
                        if let Some(time) = seconds(&token) {
                            if duration.is_none() {
                                if time < 0. {
                                    return false;
                                }
                                duration = Some(time);
                            } else if delay.is_none() {
                                delay = Some(time);
                            } else {
                                return false;
                            }
                        } else if let Some(easing) = easing(&token) {
                            if curve.replace(easing).is_some() {
                                return false;
                            }
                        } else if let Some(name) = property(&token) {
                            if property_name.replace(name).is_some() {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                    let name = property_name.unwrap_or_else(|| "all".into());
                    if name == "none" && parts.len() > 1 {
                        return false;
                    }
                    parsed.properties.push(name);
                    parsed.durations.push(duration.unwrap_or(0.));
                    parsed.delays.push(delay.unwrap_or(0.));
                    parsed.easings.push(curve.unwrap_or_default());
                }
                *self = parsed;
            }
        }
        true
    }
}

fn comma_values(value: &str) -> Option<Vec<String>> {
    if value.len() > 65536 {
        return None;
    }
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let values = parser
        .parse_comma_separated(|parser| {
            let start = parser.position();
            while !parser.is_exhausted() {
                match parser.next()?.clone() {
                    Token::Function(_) | Token::ParenthesisBlock => {
                        parser.parse_nested_block(|parser| {
                            while !parser.is_exhausted() {
                                parser.next()?;
                            }
                            Ok::<_, cssparser::ParseError<'_, ()>>(())
                        })?;
                    }
                    _ => {}
                }
            }
            let value = parser.slice_from(start).trim();
            if value.is_empty() {
                return Err(parser.new_custom_error(()));
            }
            Ok(value.to_owned())
        })
        .ok()?;
    (values.len() <= 256).then_some(values)
}
fn property(value: &str) -> Option<String> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let name = parser.expect_ident().ok()?.to_string();
    parser.expect_exhausted().ok()?;
    (!matches!(
        name.to_ascii_lowercase().as_str(),
        "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default"
    ))
    .then(|| {
        if name.starts_with("--") {
            name
        } else {
            name.to_ascii_lowercase()
        }
    })
}
fn seconds(value: &str) -> Option<f32> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let Token::Dimension { value, unit, .. } = parser.next().ok()?.clone() else {
        return None;
    };
    let value = if unit.eq_ignore_ascii_case("s") {
        value
    } else if unit.eq_ignore_ascii_case("ms") {
        value / 1000.
    } else {
        return None;
    };
    parser.expect_exhausted().ok()?;
    (value.is_finite() && value.abs() <= 86400.).then_some(value)
}
fn easing(value: &str) -> Option<Easing> {
    let value = value.to_ascii_lowercase();
    let bezier = match value.as_str() {
        "linear" => Some([0., 0., 1., 1.]),
        "ease" => Some([0.25, 0.1, 0.25, 1.]),
        "ease-in" => Some([0.42, 0., 1., 1.]),
        "ease-out" => Some([0., 0., 0.58, 1.]),
        "ease-in-out" => Some([0.42, 0., 0.58, 1.]),
        _ => None,
    };
    if let Some(curve) = bezier {
        return Some(Easing::Bezier(curve));
    }
    if value == "step-start" || value == "step-end" {
        return Some(Easing::Steps {
            count: 1,
            start: value == "step-start",
            end: value == "step-end",
        });
    }
    let (name, arguments) = value.split_once('(')?;
    let arguments = arguments.strip_suffix(')')?;
    let parts: Vec<_> = arguments.split(',').map(str::trim).collect();
    match name {
        "cubic-bezier" => {
            let values: Vec<f32> = parts
                .iter()
                .map(|value| value.parse().ok())
                .collect::<Option<_>>()?;
            let curve: [f32; 4] = values.try_into().ok()?;
            (curve.iter().all(|n| n.is_finite())
                && (0. ..=1.).contains(&curve[0])
                && (0. ..=1.).contains(&curve[2]))
            .then_some(Easing::Bezier(curve))
        }
        "steps" if parts.len() <= 2 => {
            let count = parts[0].parse::<u32>().ok()?;
            let (start, end) = match parts.get(1).copied().unwrap_or("end") {
                "start" | "jump-start" => (true, false),
                "end" | "jump-end" => (false, true),
                "jump-none" => (false, false),
                "jump-both" => (true, true),
                _ => return None,
            };
            (count > u32::from(!start && !end) && count <= 1_000_000).then_some(Easing::Steps {
                count,
                start,
                end,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transition_cascades_match_original_computed_lists() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../feishin-gpui/assets/button-custom-motion-reference.json"
        ))
        .unwrap();
        for case in cases {
            let css = format!("#button {{ {} }}", case["declarations"].as_str().unwrap());
            let sheet = crate::styles::Stylesheet::parse([css.as_str()]).unwrap();
            let mut actual = Transitions::default();
            let mut variables = std::collections::BTreeMap::new();
            let mut target = crate::styles::Target::new("button", "button", &[]);
            if case["phase"] == "hover" {
                target.states.push("hover".into());
            }
            let declarations =
                sheet.ordered_declarations_with_environment(&[target], Default::default());
            variables = crate::styles::cascade_variables(
                &variables,
                declarations
                    .iter()
                    .filter(|(name, _)| name.starts_with("--"))
                    .cloned(),
            );
            for (name, value) in declarations {
                let variable = crate::styles::is_variable_value(&value);
                let value =
                    crate::styles::variables(&value, &variables).unwrap_or_else(|| "unset".into());
                if !actual.apply(&name, &value, &Transitions::default()) && variable {
                    actual.apply(&name, "unset", &Transitions::default());
                }
            }
            let mut expected = Transitions::default();
            for (field, name) in [
                ("property", "transition-property"),
                ("duration", "transition-duration"),
                ("delay", "transition-delay"),
                ("easing", "transition-timing-function"),
            ] {
                assert!(
                    expected.apply(
                        name,
                        case["timing"][field].as_str().unwrap(),
                        &Transitions::default()
                    ),
                    "{case}"
                );
            }
            for property in [
                "background-color",
                "color",
                "border-top-color",
                "opacity",
                "width",
            ] {
                assert_eq!(
                    actual.for_property(property),
                    expected.for_property(property),
                    "{property}: {case}"
                );
            }
        }
        let mut value = Transitions::default();
        assert!(value.apply(
            "transition",
            "all 300ms linear -20ms",
            &Transitions::default()
        ));
        let before = value.clone();
        for invalid in [
            "all -1s",
            "opacity 0",
            "opacity 1s 2s 3s",
            "none, opacity 1s",
            "all 1s steps(1,jump-none)",
            "all 1s cubic-bezier(2,0,1,1)",
            "all 1s,",
            "all 1s ease ease-out",
        ] {
            assert!(
                !value.apply("transition", invalid, &Transitions::default()),
                "{invalid}"
            );
            assert_eq!(value, before);
        }
        let mut inherited = Transitions::default();
        assert!(inherited.apply("transition", "inherit", &before));
        assert_eq!(inherited, before);
    }
}
