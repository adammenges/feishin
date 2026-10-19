//! Bounded media conditions for native custom CSS.
use anyhow::{Result, ensure};

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub struct Environment {
    pub width: f32,
    pub height: f32,
    pub resolution: f32,
    pub dark: bool,
    pub hover: bool,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            width: 1024.,
            height: 768.,
            resolution: 1.,
            dark: true,
            hover: true,
        }
    }
}
#[derive(Clone, Debug)]
pub enum Condition {
    Any(Vec<Self>),
    All(Vec<Self>),
    Not(Box<Self>),
    Constant(bool),
    Feature(String),
}
impl Condition {
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(source.len() <= 4096, "CSS media condition is too long");
        let mut source = source.to_ascii_lowercase();
        while let Some(start) = source.find("/*") {
            let end = source[start + 2..].find("*/").map(|end| start + 4 + end);
            ensure!(end.is_some(), "Unterminated CSS media comment");
            source.replace_range(start..end.unwrap(), " ");
        }
        let source = source
            .split([' ', '\t', '\n', '\r', '\u{000c}'])
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        Self::parse_inner(&source, 0)
    }
    fn parse_inner(source: &str, depth: usize) -> Result<Self> {
        ensure!(depth < 16, "CSS media condition is too deeply nested");
        let source = source.trim();
        let queries = split(source, ",")?;
        if queries.len() > 1 {
            return Ok(Self::Any(
                queries
                    .into_iter()
                    .map(|query| Self::parse_inner(query, depth + 1))
                    .collect::<Result<Vec<_>>>()?,
            ));
        }
        // A media type's `not` negates the entire query, unlike a condition's `not`.
        if let Some(rest) = source.strip_prefix("not ")
            && !rest.trim_start().starts_with('(')
        {
            return Ok(Self::Not(Box::new(Self::parse_inner(rest, depth + 1)?)));
        }
        for (separator, any) in [(" or ", true), (" and ", false)] {
            let parts = split(source, separator)?;
            if parts.len() > 1 {
                ensure!(
                    parts
                        .iter()
                        .enumerate()
                        .all(|(index, part)| part.starts_with('(')
                            || (index == 0 && !any && !part.starts_with("not "))),
                    "Invalid CSS boolean media condition"
                );
                let parts = parts
                    .into_iter()
                    .map(|part| Self::parse_inner(part, depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                return Ok(if any {
                    Self::Any(parts)
                } else {
                    Self::All(parts)
                });
            }
        }
        if let Some(source) = source.strip_prefix("not ") {
            return Ok(Self::Not(Box::new(Self::parse_inner(source, depth + 1)?)));
        }
        if let Some(source) = source.strip_prefix("only ") {
            return Self::parse_inner(source, depth + 1);
        }
        if source.starts_with('(') && source.ends_with(')') {
            let inner = &source[1..source.len() - 1];
            return if inner.contains('(') {
                Self::parse_inner(inner, depth + 1)
            } else {
                Ok(Self::Feature(inner.trim().to_owned()))
            };
        }
        Ok(Self::Constant(matches!(source, "screen" | "all")))
    }
    pub fn matches(&self, env: Environment) -> bool {
        self.evaluate(env) == Some(true)
    }
    fn evaluate(&self, env: Environment) -> Option<bool> {
        match self {
            Self::Any(conditions) => combine(
                conditions.iter().map(|condition| condition.evaluate(env)),
                true,
            ),
            Self::All(conditions) => combine(
                conditions.iter().map(|condition| condition.evaluate(env)),
                false,
            ),
            Self::Not(condition) => condition.evaluate(env).map(|value| !value),
            Self::Constant(value) => Some(*value),
            Self::Feature(source) => feature(source, env),
        }
    }
}
// Unknown features remain unknown under negation, as in CSS's three-valued logic.
fn combine(values: impl Iterator<Item = Option<bool>>, any: bool) -> Option<bool> {
    let mut unknown = false;
    for value in values {
        if value == Some(any) {
            return Some(any);
        }
        unknown |= value.is_none();
    }
    if unknown { None } else { Some(!any) }
}
fn split<'a>(source: &'a str, separator: &str) -> Result<Vec<&'a str>> {
    let mut depth = 0_usize;
    let mut start = 0;
    let mut parts = vec![];
    for (index, character) in source.char_indices() {
        match character {
            '(' => {
                depth += 1;
                ensure!(depth <= 16, "CSS media condition is too deeply nested");
            }
            ')' => {
                ensure!(depth > 0, "Unbalanced CSS media condition");
                depth -= 1;
            }
            _ => {}
        }
        if depth == 0 && source[index..].starts_with(separator) {
            parts.push(source[start..index].trim());
            start = index + separator.len();
        }
    }
    ensure!(depth == 0, "Unbalanced CSS media condition");
    parts.push(source[start..].trim());
    Ok(parts)
}
fn feature(source: &str, env: Environment) -> Option<bool> {
    if let Some((name, value)) = source.split_once(':') {
        let name = name.trim();
        let value = value.trim();
        return match name {
            "prefers-color-scheme" => ["dark", "light"]
                .contains(&value)
                .then_some((value == "dark") == env.dark),
            "orientation" => ["portrait", "landscape"]
                .contains(&value)
                .then_some((value == "portrait") == (env.height >= env.width)),
            "hover" | "any-hover" => ["hover", "none"]
                .contains(&value)
                .then_some((value == "hover") == env.hover),
            "pointer" | "any-pointer" => ["fine", "coarse", "none"]
                .contains(&value)
                .then_some((value == "fine" && env.hover) || (value == "none" && !env.hover)),
            _ => {
                let (operator, name) = name
                    .strip_prefix("min-")
                    .map(|name| (">=", name))
                    .or_else(|| name.strip_prefix("max-").map(|name| ("<=", name)))
                    .unwrap_or(("=", name));
                compare(name, operator, value, env)
            }
        };
    }
    // Range syntax can put the feature on either side, including a two-sided range.
    let mut operands = vec![];
    let mut operators = vec![];
    let mut start = 0;
    let mut chars = source.char_indices().peekable();
    while let Some((index, character)) = chars.next() {
        if matches!(character, '<' | '>' | '=') {
            operands.push(source[start..index].trim());
            let end = if chars.peek().is_some_and(|(_, character)| *character == '=') {
                chars.next().unwrap().0 + 1
            } else {
                index + 1
            };
            operators.push(&source[index..end]);
            start = end;
        }
    }
    operands.push(source[start..].trim());
    if operators.is_empty() {
        return match source.trim() {
            "width" => Some(env.width > 0.),
            "height" => Some(env.height > 0.),
            "hover" | "any-hover" => Some(env.hover),
            _ => None,
        };
    }
    combine(
        operators.iter().enumerate().map(|(index, operator)| {
            let left = operands[index];
            let right = operands[index + 1];
            if axis(left, env).is_some() {
                compare(left, operator, right, env)
            } else {
                let inverse = match *operator {
                    "<" => ">",
                    "<=" => ">=",
                    ">" => "<",
                    ">=" => "<=",
                    "=" => "=",
                    _ => return None,
                };
                compare(right, inverse, left, env)
            }
        }),
        false,
    )
}
fn axis(name: &str, env: Environment) -> Option<f32> {
    match name {
        "width" => Some(env.width),
        "height" => Some(env.height),
        "resolution" => Some(env.resolution),
        "aspect-ratio" => Some(env.width / env.height),
        _ => None,
    }
}
fn compare(name: &str, operator: &str, value: &str, env: Environment) -> Option<bool> {
    let actual = axis(name, env)?;
    let expected = if name == "aspect-ratio" {
        value
            .split_once('/')
            .and_then(|(a, b)| Some(a.trim().parse::<f32>().ok()? / b.trim().parse::<f32>().ok()?))
            .or_else(|| value.parse::<f32>().ok())
    } else {
        // Media em/rem lengths use the initial browser font, independent of app font size.
        let units: &[(&str, f32)] = if name == "resolution" {
            &[
                ("dppx", 1.),
                ("dpi", 1. / 96.),
                ("dpcm", 2.54 / 96.),
                ("x", 1.),
            ]
        } else {
            &[
                ("px", 1.),
                ("rem", 16.),
                ("em", 16.),
                ("in", 96.),
                ("cm", 96. / 2.54),
                ("mm", 96. / 25.4),
                ("pt", 96. / 72.),
                ("pc", 16.),
            ]
        };
        units
            .iter()
            .find_map(|(unit, scale)| {
                value
                    .strip_suffix(unit)
                    .and_then(|number| number.trim().parse::<f32>().ok())
                    .map(|number| number * scale)
            })
            .or_else(|| (value == "0").then_some(0.))
    };
    let expected = expected.filter(|number| number.is_finite())?;
    if !actual.is_finite() {
        return None;
    }
    Some(match operator {
        "<" => actual < expected,
        "<=" => actual <= expected,
        ">" => actual > expected,
        ">=" => actual >= expected,
        "=" => actual == expected,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_conditions_match_chrome_viewports_themes_ranges_and_boolean_queries() {
        let fixtures: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/media-query-reference.json")).unwrap();
        for fixture in fixtures {
            let environment: Environment =
                serde_json::from_value(fixture["environment"].clone()).unwrap();
            for case in fixture["cases"].as_array().unwrap() {
                let query = case["query"].as_str().unwrap();
                let actual =
                    Condition::parse(query).is_ok_and(|condition| condition.matches(environment));
                assert_eq!(
                    actual,
                    case["expected"].as_bool().unwrap(),
                    "{query} {environment:?}"
                );
            }
        }
        assert!(Condition::parse(&"(".repeat(17)).is_err());
        assert!(Condition::parse(&"x".repeat(4097)).is_err());
    }
}
