//! Bounded CSS syntax parsing and selector cascade for native UI aliases.
use crate::media_queries::Condition;
pub use crate::media_queries::Environment as MediaEnvironment;
use anyhow::{Result, ensure};
use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser, Token,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterOperation {
    Blur(f32),
    Brightness(f32),
    Contrast(f32),
    Saturate(f32),
    Grayscale(f32),
    Sepia(f32),
    HueRotate(f32),
    Invert(f32),
    Opacity(f32),
    DropShadow {
        offset: [f32; 2],
        sigma: f32,
        /// None uses the element's computed text color.
        color: Option<[f32; 4]>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum BackdropFilter {
    Blur(f32),
    BlurSaturate(f32, f32),
    Chain(Vec<FilterOperation>),
    Inherit,
}

/// Parse bounded CSS filter functions in their declared order.
pub fn backdrop_filter(value: &str, font_size: f32) -> Option<BackdropFilter> {
    if value.len() > 4096 {
        return None;
    }
    let mut source = ParserInput::new(value);
    let mut input = Parser::new(&mut source);
    let first = input.next().ok()?.clone();
    if let Token::Ident(name) = &first {
        let filter = match name.to_ascii_lowercase().as_str() {
            "inherit" => BackdropFilter::Inherit,
            "none" | "initial" | "unset" | "revert" => BackdropFilter::Blur(0.),
            _ => return None,
        };
        return input.is_exhausted().then_some(filter);
    }
    let mut token = first;
    let mut operations = vec![];
    loop {
        let Token::Function(name) = token else {
            return None;
        };
        let name = name.to_ascii_lowercase();
        let operation = input
            .parse_nested_block(|input| {
                if name == "drop-shadow" {
                    let value = remainder(input);
                    return drop_shadow(&value, font_size)
                        .ok_or_else(|| input.new_custom_error::<(), ()>(()));
                }
                let amount = if input.is_exhausted() {
                    if name == "blur" || name == "hue-rotate" {
                        0.
                    } else {
                        1.
                    }
                } else {
                    match (name.as_str(), input.next()?.clone()) {
                        ("blur" | "hue-rotate", Token::Number { value: 0., .. }) => 0.,
                        ("blur", Token::Dimension { value, unit, .. }) => {
                            match unit.to_ascii_lowercase().as_str() {
                                "px" => value,
                                "em" | "rem" => value * font_size,
                                _ => return Err(input.new_custom_error::<(), ()>(())),
                            }
                        }
                        ("hue-rotate", Token::Dimension { value, unit, .. }) => {
                            match unit.to_ascii_lowercase().as_str() {
                                "deg" => value.to_radians(),
                                "grad" => value * std::f32::consts::PI / 200.,
                                "rad" => value,
                                "turn" => value * std::f32::consts::TAU,
                                _ => return Err(input.new_custom_error::<(), ()>(())),
                            }
                        }
                        ("blur" | "hue-rotate", _) => {
                            return Err(input.new_custom_error::<(), ()>(()));
                        }
                        (_, Token::Number { value, .. }) => value,
                        (_, Token::Percentage { unit_value, .. }) => unit_value,
                        _ => return Err(input.new_custom_error::<(), ()>(())),
                    }
                };
                if !amount.is_finite()
                    || amount.abs() > 10000.
                    || (name != "hue-rotate" && amount < 0.)
                    || !input.is_exhausted()
                {
                    return Err(input.new_custom_error::<(), ()>(()));
                }
                Ok(match name.as_str() {
                    "blur" => FilterOperation::Blur(amount),
                    "brightness" => FilterOperation::Brightness(amount),
                    "contrast" => FilterOperation::Contrast(amount),
                    "saturate" => FilterOperation::Saturate(amount),
                    "grayscale" => FilterOperation::Grayscale(amount.min(1.)),
                    "sepia" => FilterOperation::Sepia(amount.min(1.)),
                    "hue-rotate" => FilterOperation::HueRotate(amount),
                    "invert" => FilterOperation::Invert(amount.min(1.)),
                    "opacity" => FilterOperation::Opacity(amount.min(1.)),
                    _ => return Err(input.new_custom_error::<(), ()>(())),
                })
            })
            .ok()?;
        operations.push(operation);
        if operations.len() > 64 {
            return None;
        }
        if input.is_exhausted() {
            break;
        }
        token = input.next().ok()?.clone();
    }
    Some(match operations.as_slice() {
        [FilterOperation::Blur(sigma)] => BackdropFilter::Blur(*sigma),
        [FilterOperation::Saturate(amount)] => BackdropFilter::BlurSaturate(0., *amount),
        [
            FilterOperation::Blur(sigma),
            FilterOperation::Saturate(amount),
        ] => BackdropFilter::BlurSaturate(*sigma, *amount),
        _ => BackdropFilter::Chain(operations),
    })
}

/// A CSS text shadow. Blur radius is twice the Gaussian standard deviation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextShadow {
    pub offset: [f32; 2],
    pub blur_radius: f32,
    pub color: Option<[f32; 4]>,
}

/// Parse independent text-shadow layers, retaining currentColor until painting.
pub fn text_shadows(value: &str, font_size: f32) -> Option<Vec<TextShadow>> {
    if value.trim().eq_ignore_ascii_case("none") {
        return Some(vec![]);
    }
    let parts = component_values(value)?;
    parts
        .split(|part| part == ",")
        .map(|parts| match drop_shadow(&parts.join(" "), font_size)? {
            FilterOperation::DropShadow {
                offset,
                sigma,
                color,
            } => Some(TextShadow {
                offset,
                blur_radius: sigma,
                color,
            }),
            _ => None,
        })
        .collect()
}

fn drop_shadow(value: &str, font_size: f32) -> Option<FilterOperation> {
    let mut lengths = vec![];
    let mut color = None;
    let mut has_color = false;
    let mut lengths_finished = false;
    for part in component_values(value)? {
        let mut source = ParserInput::new(&part);
        let mut input = Parser::new(&mut source);
        let length = match input.next().ok()?.clone() {
            Token::Number { value: 0., .. } => Some(0.),
            Token::Dimension { value, unit, .. } => match unit.to_ascii_lowercase().as_str() {
                "px" => Some(value),
                "em" | "rem" => Some(value * font_size),
                _ => None,
            },
            _ => None,
        };
        if let Some(length) = length {
            if !input.is_exhausted() || !length.is_finite() || length.abs() > 10000. {
                return None;
            }
            // The two offsets and optional blur must form one contiguous list.
            if lengths_finished {
                return None;
            }
            lengths.push(length);
        } else {
            if has_color || lengths.len() == 1 {
                return None;
            }
            color = if part.eq_ignore_ascii_case("currentcolor") {
                None
            } else {
                Some(crate::appearance::color(&part).ok()?)
            };
            has_color = true;
            lengths_finished = !lengths.is_empty();
        }
    }
    let (offset, sigma) = match lengths.as_slice() {
        [x, y] => ([*x, *y], 0.),
        [x, y, sigma] if *sigma >= 0. => ([*x, *y], *sigma),
        _ => return None,
    };
    Some(FilterOperation::DropShadow {
        offset,
        sigma,
        color,
    })
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub content: String,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.content.len() <= 1_000_000, "Custom CSS is too large");
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Target {
    pub tag: String,
    pub id: String,
    pub classes: Vec<String>,
    pub attributes: BTreeMap<String, String>,
    pub states: Vec<String>,
    pub inline_declarations: Vec<(String, String)>,
    /// Known descendants outside the ancestor path, such as the window bar.
    pub children: Vec<Target>,
    /// A generated box follows its originating element in the style path.
    pub pseudo_element: Option<String>,
}
impl Target {
    pub fn pseudo(name: &str) -> Self {
        Self {
            pseudo_element: Some(name.into()),
            ..Default::default()
        }
    }
    pub fn new(tag: &str, id: &str, classes: &[&str]) -> Self {
        Self {
            tag: tag.into(),
            id: id.into(),
            classes: classes.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }
}
#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    value: String,
    important: bool,
}
#[derive(Clone, Debug)]
enum Body {
    Declaration(Declaration),
    Rule(String, Vec<Body>),
    Media(Condition, Vec<Body>),
}
struct Syntax {
    depth: usize,
}
fn remainder(input: &mut Parser<'_, '_>) -> String {
    let start = input.position();
    while input.next_including_whitespace_and_comments().is_ok() {}
    input.slice_from(start).trim().to_owned()
}

fn nested_selector(parent: &str, nested: &str) -> Result<String> {
    fn ampersands<'i>(
        input: &mut Parser<'i, '_>,
        offsets: &mut Vec<usize>,
        depth: usize,
    ) -> std::result::Result<(), ParseError<'i, ()>> {
        if depth >= 16 {
            return Err(input.new_custom_error(()));
        }
        while !input.is_exhausted() {
            let start = input.position().byte_index();
            match input.next_including_whitespace_and_comments()?.clone() {
                Token::Delim('&') => offsets.push(start),
                Token::Function(_) | Token::ParenthesisBlock => {
                    input.parse_nested_block(|input| ampersands(input, offsets, depth + 1))?
                }
                _ => {}
            }
        }
        Ok(())
    }
    let mut source = ParserInput::new(nested);
    let mut input = Parser::new(&mut source);
    let branches = input
        .parse_comma_separated(|input| Ok::<_, ParseError<'_, ()>>(remainder(input)))
        .map_err(|_| anyhow::anyhow!("Unsupported nested CSS selector: {nested}"))?;
    let parent = format!(":is({parent})");
    let mut expanded = String::new();
    for branch in branches {
        let mut source = ParserInput::new(&branch);
        let mut input = Parser::new(&mut source);
        let mut offsets = vec![];
        ampersands(&mut input, &mut offsets, 0)
            .map_err(|_| anyhow::anyhow!("Nested CSS selector is too complex"))?;
        if !expanded.is_empty() {
            expanded.push(',');
        }
        if offsets.is_empty() {
            expanded.push_str(&parent);
            expanded.push(' ');
        }
        let mut start = 0;
        for offset in offsets {
            expanded.push_str(&branch[start..offset]);
            expanded.push_str(&parent);
            start = offset + 1;
            ensure!(expanded.len() <= 4096, "Nested CSS selector is too complex");
        }
        expanded.push_str(&branch[start..]);
        ensure!(expanded.len() <= 4096, "Nested CSS selector is too complex");
    }
    Ok(expanded)
}
impl<'i> QualifiedRuleParser<'i> for Syntax {
    type Prelude = String;
    type QualifiedRule = Body;
    type Error = ();
    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<String, ParseError<'i, ()>> {
        let selector = remainder(input);
        if selector.len() > 4096 || self.depth >= 16 {
            return Err(input.new_custom_error(()));
        }
        Ok(selector)
    }
    fn parse_block<'t>(
        &mut self,
        selector: String,
        _: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Body, ParseError<'i, ()>> {
        let mut body = Syntax {
            depth: self.depth + 1,
        };
        let mut values = vec![];
        for item in RuleBodyParser::new(input, &mut body).take(4097) {
            if let Ok(item) = item {
                values.push(item);
            }
            if values.len() > 4096 {
                return Err(input.new_custom_error(()));
            }
        }
        Ok(Body::Rule(selector, values))
    }
}
impl<'i> AtRuleParser<'i> for Syntax {
    type Prelude = Condition;
    type AtRule = Body;
    type Error = ();
    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Self::Prelude, ParseError<'i, ()>> {
        if !name.eq_ignore_ascii_case("media") || self.depth >= 16 {
            return Err(input.new_custom_error(()));
        }
        Condition::parse(&remainder(input)).map_err(|_| input.new_custom_error(()))
    }
    fn parse_block<'t>(
        &mut self,
        condition: Condition,
        _: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> std::result::Result<Body, ParseError<'i, ()>> {
        let mut syntax = Syntax {
            depth: self.depth + 1,
        };
        let body: Vec<_> = RuleBodyParser::new(input, &mut syntax)
            .take(4097)
            .flatten()
            .collect();
        if body.len() > 4096 {
            return Err(input.new_custom_error(()));
        }
        Ok(Body::Media(condition, body))
    }
}
fn declaration_priority(raw: &str) -> (&str, bool) {
    raw.rsplit_once('!')
        .filter(|(_, tail)| tail.trim().eq_ignore_ascii_case("important"))
        .map_or((raw.trim(), false), |(value, _)| (value.trim(), true))
}
impl<'i> DeclarationParser<'i> for Syntax {
    type Declaration = Body;
    type Error = ();
    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _: &ParserState,
    ) -> std::result::Result<Body, ParseError<'i, ()>> {
        let raw = remainder(input);
        let (value, important) = declaration_priority(&raw);
        if name == "--" || (name.starts_with("--") && variable_count(value).is_none()) {
            return Err(input.new_custom_error(()));
        }
        Ok(Body::Declaration(Declaration {
            name: if name.starts_with("--") {
                name.to_string()
            } else {
                name.to_ascii_lowercase()
            },
            value: value.trim().to_owned(),
            important,
        }))
    }
}
impl<'i> RuleBodyItemParser<'i, Body, ()> for Syntax {
    fn parse_declarations(&self) -> bool {
        true
    }
    fn parse_qualified(&self) -> bool {
        true
    }
}

type Specificity = (u16, u16, u16);
#[derive(Clone, Debug)]
enum Test {
    Tag(String),
    Class(String),
    Id(String),
    Attribute(String, Option<String>),
    State(String),
    Root,
    Scope,
    Any(Vec<Selector>, bool),
    Has(Vec<Selector>),
}
#[derive(Clone, Debug)]
struct Compound {
    tests: Vec<Test>,
    child: bool,
}
#[derive(Clone, Debug)]
struct Selector {
    parts: Vec<Compound>,
    specificity: Specificity,
    pseudo_element: Option<String>,
}
fn selector_list(text: &str, depth: usize) -> Result<Vec<Selector>> {
    ensure!(depth < 16 && text.len() <= 4096, "Selector is too complex");
    let mut source = ParserInput::new(text);
    let mut parser = Parser::new(&mut source);
    parser
        .parse_comma_separated(|input| {
            let raw = remainder(input);
            parse_selector(&raw, depth).map_err(|_| input.new_custom_error::<(), ()>(()))
        })
        .map_err(|_| anyhow::anyhow!("Unsupported CSS selector: {text}"))
}
fn parse_selector(text: &str, depth: usize) -> Result<Selector> {
    ensure!(depth < 16 && text.len() <= 4096, "Selector is too complex");
    let mut source = ParserInput::new(text);
    let mut input = Parser::new(&mut source);
    let mut parts = vec![];
    let mut tests = vec![];
    let mut in_compound = false;
    let mut child = false;
    let mut score = (0u16, 0u16, 0u16);
    let mut pseudo_element = None;
    while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
        if pseudo_element.is_some() {
            ensure!(
                matches!(token, Token::Comment(_) | Token::WhiteSpace(_)),
                "A pseudo-element must end the selector"
            );
            continue;
        }
        match token {
            Token::Comment(_) => {}
            Token::WhiteSpace(_) => {
                if in_compound {
                    parts.push(Compound {
                        tests: std::mem::take(&mut tests),
                        child,
                    });
                    child = false;
                    in_compound = false;
                }
            }
            Token::Delim('>') => {
                if in_compound {
                    parts.push(Compound {
                        tests: std::mem::take(&mut tests),
                        child,
                    });
                }
                child = true;
                in_compound = false;
            }
            Token::Delim('*') => {
                in_compound = true;
            }
            Token::Ident(tag) => {
                tests.push(Test::Tag(tag.to_ascii_lowercase()));
                score.2 += 1;
            }
            Token::IDHash(id) => {
                tests.push(Test::Id(id.to_string()));
                score.0 += 1;
            }
            Token::Delim('.') => {
                tests.push(Test::Class(
                    input
                        .expect_ident()
                        .map_err(|_| anyhow::anyhow!("Invalid CSS class"))?
                        .to_string(),
                ));
                score.1 += 1;
            }
            Token::SquareBracketBlock => {
                let attribute = input
                    .parse_nested_block(|input| {
                        let name = input.expect_ident()?.to_string();
                        let value = if input.is_exhausted() {
                            None
                        } else {
                            input.expect_delim('=')?;
                            Some(input.expect_ident_or_string()?.to_string())
                        };
                        Ok::<_, ParseError<'_, ()>>(Test::Attribute(name, value))
                    })
                    .map_err(|_| anyhow::anyhow!("Unsupported CSS attribute selector"))?;
                tests.push(attribute);
                score.1 += 1;
            }
            Token::Colon => {
                match input
                    .next()
                    .map_err(|_| anyhow::anyhow!("Invalid CSS state"))?
                    .clone()
                {
                    Token::Colon => {
                        let name = input
                            .expect_ident()
                            .map_err(|_| anyhow::anyhow!("Invalid CSS pseudo-element"))?;
                        ensure!(
                            matches!(name.as_ref(), "before" | "after"),
                            "Unsupported CSS pseudo-element"
                        );
                        pseudo_element = Some(name.to_string());
                        score.2 += 1;
                        in_compound = true;
                    }
                    Token::Ident(name) if matches!(name.as_ref(), "before" | "after") => {
                        pseudo_element = Some(name.to_string());
                        score.2 += 1;
                        in_compound = true;
                    }
                    Token::Ident(state) if state == "root" => {
                        tests.push(Test::Root);
                        score.1 += 1;
                    }
                    Token::Ident(state) if state == "scope" => {
                        tests.push(Test::Scope);
                        score.1 += 1;
                    }
                    Token::Ident(state)
                        if matches!(
                            state.as_ref(),
                            "hover" | "active" | "focus" | "focus-visible" | "disabled" | "checked"
                        ) =>
                    {
                        tests.push(Test::State(state.to_string()));
                        score.1 += 1;
                    }
                    Token::Function(name)
                        if matches!(name.as_ref(), "is" | "where" | "not" | "has") =>
                    {
                        let raw = input
                            .parse_nested_block(|input| {
                                Ok::<_, ParseError<'_, ()>>(remainder(input))
                            })
                            .map_err(|_| anyhow::anyhow!("Invalid CSS function"))?;
                        let mut selectors = if name == "has" {
                            let mut source = ParserInput::new(&raw);
                            Parser::new(&mut source)
                                .parse_comma_separated(|input| {
                                    let relative = remainder(input);
                                    if relative.is_empty() {
                                        return Err(input.new_custom_error::<(), ()>(()));
                                    }
                                    parse_selector(&format!(":scope {relative}"), depth + 1)
                                        .map_err(|_| input.new_custom_error::<(), ()>(()))
                                })
                                .map_err(|_| anyhow::anyhow!("Unsupported relative selector"))?
                        } else {
                            selector_list(&raw, depth + 1)?
                        };
                        if name == "not" {
                            ensure!(
                                selectors.iter().all(|s| s.pseudo_element.is_none()),
                                "Pseudo-elements are invalid in :not"
                            );
                        } else if matches!(name.as_ref(), "is" | "where") {
                            // These forgiving lists discard pseudo-element branches;
                            // the remaining selectors still participate in the cascade.
                            selectors.retain(|s| s.pseudo_element.is_none());
                        }
                        ensure!(depth < 16, "Selector is too complex");
                        if name == "has" {
                            ensure!(
                                !selectors.iter().any(Selector::has_or_pseudo),
                                "Nested :has and pseudo-elements are invalid in :has"
                            );
                        }
                        if name != "where" {
                            let nested = selectors
                                .iter()
                                .map(|s| s.specificity)
                                .max()
                                .unwrap_or_default();
                            score.0 += nested.0;
                            // The implicit :scope used to anchor relative selectors
                            // is not part of :has() specificity.
                            score.1 += nested.1 - u16::from(name == "has");
                            score.2 += nested.2;
                        }
                        tests.push(if name == "has" {
                            Test::Has(selectors)
                        } else {
                            Test::Any(selectors, name == "not")
                        });
                    }
                    _ => anyhow::bail!("Unsupported CSS state"),
                }
            }
            _ => anyhow::bail!("Unsupported CSS selector token"),
        }
        if !tests.is_empty() {
            in_compound = true;
        }
        ensure!(parts.len() <= 64, "Selector is too complex");
    }
    // The universal selector has no tests but still denotes one element.
    if in_compound {
        parts.push(Compound { tests, child });
    }
    ensure!(!parts.is_empty(), "Empty CSS selector");
    Ok(Selector {
        parts,
        specificity: score,
        pseudo_element,
    })
}
impl Selector {
    fn has_or_pseudo(&self) -> bool {
        self.pseudo_element.is_some()
            || self
                .parts
                .iter()
                .flat_map(|part| &part.tests)
                .any(|test| match test {
                    Test::Has(_) => true,
                    Test::Any(selectors, _) => selectors.iter().any(Self::has_or_pseudo),
                    _ => false,
                })
    }
    fn matches(&self, path: &[Target]) -> bool {
        self.matches_scoped(path, None)
    }
    fn matches_scoped(&self, path: &[Target], scope: Option<usize>) -> bool {
        let Some(target) = path.last() else {
            return false;
        };
        if self.pseudo_element != target.pseudo_element {
            return false;
        }
        let path = if self.pseudo_element.is_some() {
            &path[..path.len() - 1]
        } else {
            path
        };
        !path.is_empty() && self.matches_at(self.parts.len() - 1, path.len() - 1, path, scope)
    }
    fn matches_at(&self, part: usize, node: usize, path: &[Target], scope: Option<usize>) -> bool {
        let target = &path[node];
        if !self.parts[part].tests.iter().all(|test| match test {
            Test::Tag(tag) => target.tag == *tag,
            Test::Class(class) => target.classes.contains(class),
            Test::Id(id) => target.id == *id,
            Test::Attribute(name, value) => target
                .attributes
                .get(name)
                .is_some_and(|actual| value.as_ref().is_none_or(|value| value == actual)),
            Test::State(state) => target.states.contains(state),
            Test::Root => node == 0,
            Test::Scope => node == scope.unwrap_or(0),
            Test::Any(selectors, invert) => {
                selectors
                    .iter()
                    .any(|s| s.matches_scoped(&path[..=node], scope))
                    != *invert
            }
            Test::Has(selectors) => {
                fn descendants(
                    selectors: &[Selector],
                    path: &mut Vec<Target>,
                    scope: usize,
                ) -> bool {
                    if path.len() > 64 {
                        return false;
                    }
                    let children = path.last().unwrap().children.clone();
                    children.into_iter().any(|child| {
                        path.push(child);
                        let matched = selectors
                            .iter()
                            .any(|s| s.matches_scoped(path, Some(scope)))
                            || descendants(selectors, path, scope);
                        path.pop();
                        matched
                    })
                }
                (node..path.len()).any(|end| {
                    (end > node
                        && selectors
                            .iter()
                            .any(|s| s.matches_scoped(&path[..=end], Some(node))))
                        || descendants(selectors, &mut path[..=end].to_vec(), node)
                })
            }
        }) {
            return false;
        }
        if part == 0 {
            return true;
        }
        if node == 0 {
            return false;
        }
        if self.parts[part].child {
            self.matches_at(part - 1, node - 1, path, scope)
        } else {
            (0..node)
                .rev()
                .any(|ancestor| self.matches_at(part - 1, ancestor, path, scope))
        }
    }
}
#[derive(Clone, Debug)]
struct Rule {
    selectors: Vec<Selector>,
    declaration: Declaration,
    conditions: Vec<Condition>,
}
#[derive(Clone, Debug, Default)]
pub struct Stylesheet {
    rules: Vec<Rule>,
    pub warnings: Vec<String>,
}
impl Stylesheet {
    pub fn property_values(&self) -> impl Iterator<Item = (&str, &str)> {
        self.rules.iter().map(|rule| {
            (
                rule.declaration.name.as_str(),
                rule.declaration.value.as_str(),
            )
        })
    }
    pub fn property_names(&self) -> std::collections::BTreeSet<&str> {
        self.rules
            .iter()
            .map(|rule| rule.declaration.name.as_str())
            .collect()
    }
    pub fn parse<'a>(sources: impl IntoIterator<Item = &'a str>) -> Result<Self> {
        let mut sheet = Self::default();
        let mut size = 0;
        for source in sources {
            size += source.len();
            ensure!(size <= 8_000_000, "Theme CSS is too large");
            let mut source = ParserInput::new(source);
            let mut input = Parser::new(&mut source);
            let mut syntax = Syntax { depth: 0 };
            for item in StyleSheetParser::new(&mut input, &mut syntax).take(4097) {
                match item {
                    Ok(Body::Rule(selector, body)) => sheet.flatten(&selector, body, &[])?,
                    Ok(Body::Media(condition, body)) => sheet.media(body, &[condition])?,
                    _ => sheet
                        .warnings
                        .push("Skipped an unsupported CSS rule".into()),
                }
            }
        }
        sheet.warnings.sort();
        sheet.warnings.dedup();
        Ok(sheet)
    }
    fn media(&mut self, body: Vec<Body>, conditions: &[Condition]) -> Result<()> {
        for item in body {
            match item {
                Body::Rule(selector, body) => self.flatten(&selector, body, conditions)?,
                Body::Media(condition, body) => {
                    let mut conditions = conditions.to_vec();
                    conditions.push(condition);
                    self.media(body, &conditions)?;
                }
                _ => self
                    .warnings
                    .push("Skipped a declaration outside a CSS selector".into()),
            }
        }
        Ok(())
    }
    fn flatten(&mut self, selector: &str, body: Vec<Body>, conditions: &[Condition]) -> Result<()> {
        let selectors = match selector_list(selector, 0) {
            Ok(s) => s,
            Err(error) => {
                self.warnings.push(error.to_string());
                return Ok(());
            }
        };
        for item in body {
            ensure!(self.rules.len() <= 4096, "Too many native CSS declarations");
            match item {
                Body::Declaration(declaration) => self.rules.push(Rule {
                    selectors: selectors.clone(),
                    declaration,
                    conditions: conditions.to_vec(),
                }),
                Body::Rule(nested, body) => {
                    // Only the parent list shares maximum specificity. Each child
                    // branch keeps its own specificity and implicit nesting selector.
                    let nested = match nested_selector(selector, &nested) {
                        Ok(nested) => nested,
                        Err(error) => {
                            self.warnings.push(error.to_string());
                            continue;
                        }
                    };
                    self.flatten(&nested, body, conditions)?;
                }
                Body::Media(condition, body) => {
                    let mut conditions = conditions.to_vec();
                    conditions.push(condition);
                    self.flatten(selector, body, &conditions)?;
                }
            }
        }
        Ok(())
    }
    pub fn declarations(&self, path: &[Target]) -> BTreeMap<String, String> {
        self.declarations_with_environment(path, MediaEnvironment::default())
    }
    pub fn declarations_with_environment(
        &self,
        path: &[Target],
        environment: MediaEnvironment,
    ) -> BTreeMap<String, String> {
        self.ordered_declarations_with_environment(path, environment)
            .into_iter()
            .collect()
    }
    /// Apply lower-priority declarations first so shorthands and their longhands
    /// retain source order, specificity and !important precedence.
    pub fn ordered_declarations_with_environment(
        &self,
        path: &[Target],
        environment: MediaEnvironment,
    ) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for (index, rule) in self.rules.iter().enumerate() {
            if !rule
                .conditions
                .iter()
                .all(|condition| condition.matches(environment))
            {
                continue;
            }
            if let Some(score) = rule
                .selectors
                .iter()
                .filter(|selector| selector.matches(path))
                .map(|selector| selector.specificity)
                .max()
            {
                let priority = (rule.declaration.important, false, score, index);
                // Retain earlier values so property parsers can ignore invalid later declarations.
                found.push((
                    priority,
                    rule.declaration.name.clone(),
                    rule.declaration.value.clone(),
                ));
            }
        }
        if let Some(target) = path.last() {
            for (index, (name, value)) in target.inline_declarations.iter().enumerate() {
                let (value, important) = declaration_priority(value);
                found.push((
                    (important, true, (0, 0, 0), index),
                    name.clone(),
                    value.into(),
                ));
            }
        }
        found.sort_by_key(|(priority, _, _)| *priority);
        found
            .into_iter()
            .map(|(_, name, value)| (name, value))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn css_calc_preserves_dimensions_precedence_and_bounds() {
        for (value, pixels, fraction) in [
            ("50%", 0., 0.5),
            ("0%", 0., 0.),
            ("calc(50% - 10px)", -10., 0.5),
            ("calc((50% + 20px) * 2)", 40., 1.),
            ("calc(50% / 2 - 1rem)", -16., 0.25),
            ("calc(2em - 10%)", 24., -0.1),
            ("calc(50% * 2px / 1px)", 0., 1.),
            ("calc(0% + 3px)", 3., 0.),
        ] {
            let actual = super::length_percentage(value, 12., 16.).unwrap();
            assert!((actual.0 - pixels).abs() < 0.0001, "{value}");
            assert!((actual.1 - fraction).abs() < 0.0001, "{value}");
            assert!(
                super::length(value, 12., 16.).is_none(),
                "Strict length: {value}"
            );
        }
        for value in [
            "calc(50% + 2)",
            "calc(50% * 50%)",
            "calc(50% / 0)",
            "calc(50%+ 2px)",
            "calc(50% +2px)",
            "calc(50% * 3px)",
            "0% 2px",
        ] {
            assert!(
                super::length_percentage(value, 12., 16.).is_none(),
                "{value}"
            );
        }
        for (value, expected) in [
            ("calc(.8125rem * 1.3)", 16.9),
            ("calc(calc(1.125rem * 1.3) / 1.5)", 15.6),
            ("calc(2em + 1rem * 2)", 56.),
            ("calc((2em + 1rem) * 2)", 80.),
            ("calc(12px * 3px / 2px)", 18.),
            ("calc(1px - 3px)", -2.),
            ("0", 0.),
            ("0.0", 0.),
            ("-.0", 0.),
            ("+0e2", 0.),
        ] {
            assert!(
                (super::length(value, 12., 16.).unwrap() - expected).abs() < 0.0001,
                "{value}"
            );
        }
        for value in [
            "12",
            "calc(0)",
            "calc(1px + 2)",
            "calc(1px * 2px)",
            "calc(2px / 1px)",
            "calc(1px / 0)",
            "calc(1px+ 2px)",
            "calc(1px +2px)",
            "calc(1px +)",
            "calc(1px 2px)",
            "calc(1px) 2px",
            "calc(1px * 10001)",
            "calc(NaN * 1px)",
            "(1px)",
        ] {
            assert!(super::length(value, 12., 16.).is_none(), "{value}");
        }
        assert!(
            super::length(
                &format!("{}1px{}", "calc(".repeat(40), ")".repeat(40)),
                12.,
                16.
            )
            .is_none()
        );
    }

    #[test]
    fn generated_boxes_and_relational_selectors_match_original_cascade() {
        use super::{Stylesheet, Target};
        fn build_target(value: &serde_json::Value) -> Target {
            let mut target = Target::new(
                value["tag"].as_str().unwrap(),
                value["id"].as_str().unwrap(),
                &[],
            );
            target.classes = value["classes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().into())
                .collect();
            target.attributes = value["attributes"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.clone(), value.as_str().unwrap().into()))
                .collect();
            target.children = value["children"]
                .as_array()
                .unwrap()
                .iter()
                .map(build_target)
                .collect();
            target
        }
        fn path(target: &Target, id: &str) -> Option<Vec<Target>> {
            if target.id == id {
                return Some(vec![target.clone()]);
            }
            target.children.iter().find_map(|child| {
                let mut tail = path(child, id)?;
                tail.insert(0, target.clone());
                Some(tail)
            })
        }
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/css-relational-selectors-reference.json"
        ))
        .unwrap();
        for scene in reference.as_array().unwrap() {
            let tree = build_target(&scene["tree"]);
            for case in scene["cases"].as_array().unwrap() {
                let sheet = Stylesheet::parse([case["source"].as_str().unwrap()]).unwrap();
                let mut path = path(&tree, case["id"].as_str().unwrap()).unwrap();
                if let Some(pseudo) = case["pseudo"].as_str().filter(|s| !s.is_empty()) {
                    path.push(Target::pseudo(pseudo));
                }
                let actual = sheet
                    .declarations(&path)
                    .get("opacity")
                    .map(|s| s.parse::<f64>().unwrap())
                    .unwrap_or(1.);
                assert_eq!(
                    actual,
                    case["expected"].as_f64().unwrap(),
                    "{}: {case}",
                    scene["mode"]
                );
            }
        }
        let glass = crate::appearance::builtins().remove("glassyDark").unwrap();
        let sheet = Stylesheet::parse(glass.stylesheets.iter().map(String::as_str)).unwrap();
        assert!(sheet.warnings.is_empty(), "{:?}", sheet.warnings);
        let nested = format!("{}i{}{{opacity:0}}", ":has(".repeat(40), ")".repeat(40));
        assert!(
            !Stylesheet::parse([nested.as_str()])
                .unwrap()
                .warnings
                .is_empty()
        );
    }
    #[test]
    fn filter_chains_preserve_order_units_and_validation() {
        use super::{BackdropFilter, FilterOperation, backdrop_filter};
        assert_eq!(
            backdrop_filter("saturate(2) blur(3px) invert(180%)", 16.),
            Some(BackdropFilter::Chain(vec![
                FilterOperation::Saturate(2.),
                FilterOperation::Blur(3.),
                FilterOperation::Invert(1.)
            ]))
        );
        assert_eq!(
            backdrop_filter("brightness() contrast(50%) opacity(2)", 16.),
            Some(BackdropFilter::Chain(vec![
                FilterOperation::Brightness(1.),
                FilterOperation::Contrast(0.5),
                FilterOperation::Opacity(1.)
            ]))
        );
        for (unit, expected) in [
            ("180deg", std::f32::consts::PI),
            ("200grad", std::f32::consts::PI),
            (".5turn", std::f32::consts::PI),
            ("-1rad", -1.),
        ] {
            let Some(BackdropFilter::Chain(operations)) =
                backdrop_filter(&format!("hue-rotate({unit})"), 16.)
            else {
                panic!("{unit}")
            };
            let FilterOperation::HueRotate(actual) = operations[0] else {
                panic!("{unit}")
            };
            assert!((actual - expected).abs() < 0.000001);
        }
        for invalid in [
            "blur(1)",
            "brightness(-1)",
            "contrast(1px)",
            "invert(2 3)",
            "opacity(NaN)",
            "hue-rotate(2)",
            "blur(1px) none",
            "sepia(1), invert(1)",
            "blur(-1px)",
            "unknown()",
            "brightness(10001)",
        ] {
            assert_eq!(backdrop_filter(invalid, 16.), None, "{invalid}");
        }
        assert!(backdrop_filter(&"invert(0) ".repeat(64), 16.).is_some());
        assert!(backdrop_filter(&"invert(0) ".repeat(65), 16.).is_none());
        assert!(backdrop_filter(&" ".repeat(4097), 16.).is_none());
        for source in [
            "drop-shadow(red -1em .5rem 3px)",
            "drop-shadow(-1em .5rem 3px red)",
        ] {
            assert_eq!(
                backdrop_filter(source, 16.),
                Some(BackdropFilter::Chain(vec![FilterOperation::DropShadow {
                    offset: [-16., 8.],
                    sigma: 3.,
                    color: Some([1., 0., 0., 1.])
                }]))
            );
        }
        for source in ["drop-shadow(0 0)", "drop-shadow(currentColor 0 0)"] {
            assert_eq!(
                backdrop_filter(source, 16.),
                Some(BackdropFilter::Chain(vec![FilterOperation::DropShadow {
                    offset: [0., 0.],
                    sigma: 0.,
                    color: None
                }]))
            );
        }
        for invalid in [
            "drop-shadow()",
            "drop-shadow(red)",
            "drop-shadow(0)",
            "drop-shadow(0 0 -1px)",
            "drop-shadow(0 0 1px 2px)",
            "drop-shadow(0 red 0)",
            "drop-shadow(0 0 red 1px)",
            "drop-shadow(0 0 2%)",
            "drop-shadow(1 2)",
            "drop-shadow(red 0 0 blue)",
            "drop-shadow(0 0 10001px)",
        ] {
            assert_eq!(backdrop_filter(invalid, 16.), None, "{invalid}");
        }
    }

    use super::*;
    #[test]
    fn conditional_rules_preserve_nested_selectors_variables_and_cascade() {
        let sheet = Stylesheet::parse(["
            body { --size: 1px; }
            @media (width <= 800px) { body { --size: 20px; button { color: red; } } }
            body { @media (prefers-color-scheme: dark) { & > button { color: blue !important; } } }
        "])
        .unwrap();
        assert!(sheet.warnings.is_empty(), "{:?}", sheet.warnings);
        let root = Target::new("body", "", &[]);
        let button = Target::new("button", "", &[]);
        let small = MediaEnvironment {
            width: 640.,
            dark: false,
            ..Default::default()
        };
        assert_eq!(
            sheet.declarations_with_environment(std::slice::from_ref(&root), small)["--size"],
            "20px"
        );
        assert_eq!(
            sheet.declarations_with_environment(&[root.clone(), button.clone()], small)["color"],
            "red"
        );
        let large = MediaEnvironment {
            width: 1000.,
            dark: true,
            ..Default::default()
        };
        assert_eq!(
            sheet.declarations_with_environment(std::slice::from_ref(&root), large)["--size"],
            "1px"
        );
        assert_eq!(
            sheet.declarations_with_environment(&[root, button], large)["color"],
            "blue"
        );
    }
    #[test]
    fn inline_style_precedence_preserves_important_and_variables() {
        let sheet = Stylesheet::parse([
            "#heading.title { font-weight:300; font-size:32px !important; --size:40px; color:red !important; }",
        ]).unwrap();
        let mut target = Target::new("h1", "heading", &["title"]);
        target.inline_declarations = vec![
            ("font-weight".into(), "900".into()),
            ("font-size".into(), "28px".into()),
            ("--size".into(), "28px".into()),
            ("color".into(), "blue !IMPORTANT ".into()),
        ];
        let values = sheet.declarations(&[target]);
        assert_eq!(values["font-weight"], "900");
        assert_eq!(values["font-size"], "32px");
        assert_eq!(values["--size"], "28px");
        assert_eq!(values["color"], "blue");
    }
    #[test]
    fn css_cascade_parses_nesting_comments_specificity_and_safe_recovery() {
        let css = r#"/* } ; */ .panel { color: red; button, input { border-radius: 12px; } & > .art:not(.ag-cell *) {border-radius:18px} color: blue; }
            #play { color: yellow } button { color: green !IMPORTANT; background: rgb(0 0 0 / 40%); } .panel button { color: purple; } @import url(https://invalid.example/ignored.css);
            :where(#play) {opacity:.8} button[data-variant='filled'] {border:0} .panel { font-family:"A; } B"; }
        "#;
        let sheet = Stylesheet::parse([css]).unwrap();
        let root = Target::new("div", "", &["panel"]);
        let mut button = Target::new("button", "play", &[]);
        button
            .attributes
            .insert("data-variant".into(), "filled".into());
        let button = sheet.declarations(&[root.clone(), button]);
        assert_eq!(button["color"], "green");
        assert_eq!(button["border-radius"], "12px");
        assert_eq!(button["border"], "0");
        assert_eq!(button["opacity"], ".8");
        assert_eq!(
            sheet.declarations(std::slice::from_ref(&root))["color"],
            "blue"
        );
        assert_eq!(
            sheet.declarations(std::slice::from_ref(&root))["font-family"],
            r#""A; } B""#
        );
        assert_eq!(
            sheet.declarations(&[root.clone(), Target::new("img", "", &["art"])])["border-radius"],
            "18px"
        );
        assert!(
            !sheet
                .declarations(&[
                    root,
                    Target::new("div", "", &["ag-cell"]),
                    Target::new("img", "", &["art"])
                ])
                .contains_key("border-radius")
        );
        assert!(!sheet.warnings.is_empty());
        let glass = crate::appearance::builtins().remove("glassyDark").unwrap();
        let glass = Stylesheet::parse(glass.stylesheets.iter().map(String::as_str)).unwrap();
        assert_eq!(
            glass.declarations(&[Target::new("div", "", &["fs-player-bar-module-container"])])["background"],
            "rgb(0 0 0 / 40%)"
        );
        assert_eq!(
            glass.declarations(&[
                Target::new("div", "", &["fs-modal-module-content"]),
                Target::new("button", "", &[])
            ])["border-radius"],
            "12px"
        );
        assert!(Stylesheet::parse(["a{}".repeat(3_000_000).as_str()]).is_err());
    }
}

/// Identify syntactically valid variable references, including escaped names.
pub fn is_variable_value(value: &str) -> bool {
    variable_count(value).is_some_and(|count| count > 0)
}

fn variable_count(value: &str) -> Option<usize> {
    fn scan<'i>(
        input: &mut Parser<'i, '_>,
        count: &mut usize,
        depth: usize,
    ) -> Result<(), ParseError<'i, ()>> {
        if depth >= 16 {
            return Err(input.new_custom_error(()));
        }
        while !input.is_exhausted() {
            match input.next()?.clone() {
                Token::Function(name) => input.parse_nested_block(|input| {
                    if name.eq_ignore_ascii_case("var") {
                        let key = input.expect_ident()?;
                        if !key.starts_with("--") || key.len() == 2 || *count >= 1024 {
                            return Err(input.new_custom_error(()));
                        }
                        if !input.is_exhausted() {
                            input.expect_comma()?;
                        }
                        *count += 1;
                    }
                    scan(input, count, depth + 1)
                })?,
                Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                    input.parse_nested_block(|input| scan(input, count, depth + 1))?
                }
                Token::BadString(_) | Token::BadUrl(_) => return Err(input.new_custom_error(())),
                _ => {}
            }
        }
        Ok(())
    }
    if value.len() > 65_536 {
        return None;
    }
    let mut source = ParserInput::new(value);
    let mut input = Parser::new(&mut source);
    let mut count = 0;
    scan(&mut input, &mut count, 0).ok()?;
    Some(count)
}

/// Compute custom properties on their defining element before inheritance.
pub fn cascade_variables(
    inherited: &BTreeMap<String, String>,
    declarations: impl IntoIterator<Item = (String, String)>,
) -> BTreeMap<String, String> {
    let mut vars = inherited.clone();
    for (key, value) in declarations {
        if !key.starts_with("--") || key == "--" {
            continue;
        }
        let keyword = {
            let mut source = ParserInput::new(&value);
            let mut parser = Parser::new(&mut source);
            parser
                .expect_ident_cloned()
                .ok()
                .filter(|_| parser.is_exhausted())
                .map(|value| value.to_ascii_lowercase())
        };
        match keyword.as_deref() {
            Some("initial") => {
                vars.remove(&key);
            }
            Some("inherit" | "unset" | "revert" | "revert-layer") => {}
            _ => {
                vars.insert(key, value);
            }
        }
    }
    vars.iter()
        .filter_map(|(key, value)| {
            substitute_variables(value, &vars, Some(key)).map(|value| (key.clone(), value))
        })
        .collect()
}

/// Expand inherited CSS variables, including fallbacks, with bounded recursion.
pub fn variables(value: &str, vars: &BTreeMap<String, String>) -> Option<String> {
    substitute_variables(value, vars, None)
}

fn substitute_variables(
    value: &str,
    vars: &BTreeMap<String, String>,
    definition: Option<&str>,
) -> Option<String> {
    use cssparser::TokenSerializationType;
    fn boundaries(value: &str) -> (TokenSerializationType, TokenSerializationType) {
        let mut source = ParserInput::new(value);
        let mut input = Parser::new(&mut source);
        let mut first = TokenSerializationType::Nothing;
        let mut last = first;
        while let Ok(token) = input.next_including_whitespace_and_comments() {
            let kind = token.serialization_type();
            first.set_if_nothing(kind);
            last = if matches!(
                token,
                Token::Function(_)
                    | Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock
            ) {
                TokenSerializationType::Other
            } else {
                kind
            };
        }
        (first, last)
    }
    let mut invalid = BTreeSet::new();
    fn expand(
        value: &str,
        vars: &BTreeMap<String, String>,
        invalid: &mut BTreeSet<String>,
        stack: &mut Vec<String>,
        depth: usize,
    ) -> Option<String> {
        use cssparser::ToCss;
        if depth >= 16 || value.len() > 65_536 {
            return None;
        }
        variable_count(value)?;
        let mut source = ParserInput::new(value);
        let mut input = Parser::new(&mut source);
        let mut output = String::new();
        let mut previous = TokenSerializationType::Nothing;
        while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
            let kind = token.serialization_type();
            let (piece, first, last) = match token {
                Token::Function(name) => {
                    let inner = input
                        .parse_nested_block(|input| Ok::<_, ParseError<'_, ()>>(remainder(input)))
                        .ok()?;
                    if name.eq_ignore_ascii_case("var") {
                        let mut source = ParserInput::new(&inner);
                        let mut parser = Parser::new(&mut source);
                        let key = parser.expect_ident().ok()?.to_string();
                        if !key.starts_with("--") || key.len() == 2 {
                            return None;
                        }
                        let fallback = if parser.is_exhausted() {
                            None
                        } else {
                            parser.expect_comma().ok()?;
                            Some(remainder(&mut parser))
                        };
                        let resolved =
                            if let Some(start) = stack.iter().position(|entry| entry == &key) {
                                invalid.extend(stack[start..].iter().cloned());
                                None
                            } else if invalid.contains(&key) {
                                None
                            } else if let Some(value) = vars.get(&key) {
                                stack.push(key.clone());
                                let expanded = expand(value, vars, invalid, stack, depth + 1);
                                stack.pop();
                                // An internal fallback cannot repair a cyclic definition.
                                expanded.filter(|_| !invalid.contains(&key))
                            } else {
                                None
                            };
                        let value = resolved.or_else(|| {
                            fallback.and_then(|fallback| {
                                expand(&fallback, vars, invalid, stack, depth + 1)
                            })
                        })?;
                        let (first, last) = boundaries(&value);
                        (value, first, last)
                    } else {
                        let mut value = Token::Function(name).to_css_string();
                        value.push_str(&expand(&inner, vars, invalid, stack, depth + 1)?);
                        value.push(')');
                        (value, kind, TokenSerializationType::Other)
                    }
                }
                Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                    let (open, close) = match token {
                        Token::ParenthesisBlock => ('(', ')'),
                        Token::SquareBracketBlock => ('[', ']'),
                        _ => ('{', '}'),
                    };
                    let inner = input
                        .parse_nested_block(|input| Ok::<_, ParseError<'_, ()>>(remainder(input)))
                        .ok()?;
                    let value = format!(
                        "{open}{}{close}",
                        expand(&inner, vars, invalid, stack, depth + 1)?
                    );
                    (value, kind, TokenSerializationType::Other)
                }
                _ => (token.to_css_string(), kind, kind),
            };
            if previous.needs_separator_when_before(first) {
                // Substitution preserves token boundaries, even without whitespace.
                output.push_str("/**/");
            }
            output.push_str(&piece);
            if last != TokenSerializationType::Nothing {
                previous = last;
            }
            if output.len() > 65_536 {
                return None;
            }
        }
        Some(output)
    }
    let mut stack = definition.into_iter().map(str::to_owned).collect();
    let result = expand(value, vars, &mut invalid, &mut stack, 0);
    result.filter(|_| definition.is_none_or(|key| !invalid.contains(key)))
}
/// Resolve font-relative length tokens without changing strings, URLs or percentages.
pub fn font_relative_lengths(value: &str, font: f32, rem: f32) -> Option<String> {
    if !font.is_finite() || !rem.is_finite() {
        return None;
    }
    dimension_lengths(value, &|unit| match unit.to_ascii_lowercase().as_str() {
        "em" => Some(font),
        "rem" => Some(rem),
        _ => None,
    })
}

/// Desktop viewport units share the native content viewport, without browser chrome.
pub fn viewport_lengths(value: &str, viewport: MediaEnvironment) -> Option<String> {
    if !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width < 0.
        || viewport.height < 0.
    {
        return None;
    }
    dimension_lengths(value, &|unit| {
        Some(match unit.to_ascii_lowercase().as_str() {
            "vw" | "svw" | "lvw" | "dvw" => viewport.width / 100.,
            "vh" | "svh" | "lvh" | "dvh" => viewport.height / 100.,
            "vmin" | "svmin" | "lvmin" | "dvmin" => viewport.width.min(viewport.height) / 100.,
            "vmax" | "svmax" | "lvmax" | "dvmax" => viewport.width.max(viewport.height) / 100.,
            _ => return None,
        })
    })
}

fn dimension_lengths(value: &str, multiplier: &dyn Fn(&str) -> Option<f32>) -> Option<String> {
    use cssparser::ToCss;
    fn resolve(
        input: &mut Parser<'_, '_>,
        multiplier: &dyn Fn(&str) -> Option<f32>,
        depth: usize,
    ) -> Option<String> {
        if depth > 32 {
            return None;
        }
        let mut output = String::new();
        while !input.is_exhausted() {
            let token = input.next_including_whitespace_and_comments().ok()?.clone();
            match token {
                Token::Dimension {
                    value, ref unit, ..
                } if multiplier(unit).is_some() => {
                    let value = value * multiplier(unit)?;
                    if !value.is_finite() || value.abs() > 10000. {
                        return None;
                    }
                    output.push_str(&format!("{value}px"));
                }
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    let close = match &token {
                        Token::SquareBracketBlock => ']',
                        Token::CurlyBracketBlock => '}',
                        _ => ')',
                    };
                    output.push_str(&token.to_css_string());
                    let inner = input
                        .parse_nested_block(|input| {
                            resolve(input, multiplier, depth + 1)
                                .ok_or_else(|| input.new_custom_error::<(), ()>(()))
                        })
                        .ok()?;
                    output.push_str(&inner);
                    output.push(close);
                }
                _ => output.push_str(&token.to_css_string()),
            }
            if output.len() > 65536 {
                return None;
            }
        }
        Some(output)
    }
    if value.len() > 65536 {
        return None;
    }
    resolve(
        &mut Parser::new(&mut ParserInput::new(value)),
        multiplier,
        0,
    )
}
/// Resolve bounded CSS lengths and calc arithmetic after variable substitution.
/// A dimension is retained through arithmetic so invalid length/number sums and
/// products cannot silently become usable layout values.
pub fn length(value: &str, font: f32, rem: f32) -> Option<f32> {
    length_components(value, font, rem, false).map(|(pixels, _)| pixels)
}

/// Resolve an affine CSS length as pixels plus a fraction of its reference size.
/// Percentages retain their length dimension through calc arithmetic.
pub fn length_percentage(value: &str, font: f32, rem: f32) -> Option<(f32, f32)> {
    length_components(value, font, rem, true)
}

fn length_components(value: &str, font: f32, rem: f32, percentages: bool) -> Option<(f32, f32)> {
    #[derive(Clone, Copy)]
    struct Value(f64, f64, i32);
    fn expression(
        input: &mut Parser<'_, '_>,
        font: f32,
        rem: f32,
        percentages: bool,
        depth: usize,
    ) -> Option<Value> {
        let mut value = product(input, font, rem, percentages, depth)?;
        loop {
            let operator = input.try_parse(|input| {
                let before = input.position();
                input.skip_whitespace();
                if before == input.position() {
                    return Err(input.new_custom_error::<(), ()>(()));
                }
                let operator = match input.next()?.clone() {
                    Token::Delim(operator @ ('+' | '-')) => operator,
                    _ => return Err(input.new_custom_error::<(), ()>(())),
                };
                if !matches!(input.next_including_whitespace()?, Token::WhiteSpace(_)) {
                    return Err(input.new_custom_error::<(), ()>(()));
                }
                Ok(operator)
            });
            let Ok(operator) = operator else { break };
            let right = product(input, font, rem, percentages, depth)?;
            if value.2 != right.2 {
                return None;
            }
            value.0 += if operator == '+' { right.0 } else { -right.0 };
            value.1 += if operator == '+' { right.1 } else { -right.1 };
        }
        Some(value)
    }
    fn product(
        input: &mut Parser<'_, '_>,
        font: f32,
        rem: f32,
        percentages: bool,
        depth: usize,
    ) -> Option<Value> {
        let mut value = atom(input, font, rem, percentages, depth)?;
        loop {
            let operator = input.try_parse(|input| match input.next()?.clone() {
                Token::Delim(operator @ ('*' | '/')) => Ok(operator),
                _ => Err(input.new_custom_error::<(), ()>(())),
            });
            let Ok(operator) = operator else { break };
            let right = atom(input, font, rem, percentages, depth)?;
            if operator == '*' {
                // A quadratic percentage cannot be represented by pixels plus a fraction.
                if value.1 != 0. && right.1 != 0. {
                    return None;
                }
                value = Value(
                    value.0 * right.0,
                    value.1 * right.0 + value.0 * right.1,
                    value.2 + right.2,
                );
            } else {
                if right.1 != 0. {
                    return None;
                }
                value = Value(value.0 / right.0, value.1 / right.0, value.2 - right.2);
            }
            if !value.0.is_finite() || !value.1.is_finite() || value.2.abs() > 32 {
                return None;
            }
        }
        Some(value)
    }
    fn atom(
        input: &mut Parser<'_, '_>,
        font: f32,
        rem: f32,
        percentages: bool,
        depth: usize,
    ) -> Option<Value> {
        if depth > 32 {
            return None;
        }
        match input.next().ok()?.clone() {
            Token::Number { value, .. } => Some(Value(f64::from(value), 0., 0)),
            Token::Percentage { unit_value, .. } if percentages => {
                Some(Value(0., f64::from(unit_value), 1))
            }
            Token::Dimension { value, unit, .. } => {
                let multiplier = match unit.to_ascii_lowercase().as_str() {
                    "px" => 1.,
                    "em" => font,
                    "rem" => rem,
                    _ => return None,
                };
                Some(Value(f64::from(value) * f64::from(multiplier), 0., 1))
            }
            Token::Function(name) if name.eq_ignore_ascii_case("calc") => input
                .parse_nested_block(|input| {
                    expression(input, font, rem, percentages, depth + 1)
                        .ok_or_else(|| input.new_custom_error::<(), ()>(()))
                })
                .ok(),
            Token::ParenthesisBlock if depth > 0 => input
                .parse_nested_block(|input| {
                    expression(input, font, rem, percentages, depth + 1)
                        .ok_or_else(|| input.new_custom_error::<(), ()>(()))
                })
                .ok(),
            _ => None,
        }
    }
    if value.len() > 65536 || !font.is_finite() || !rem.is_finite() {
        return None;
    }
    let mut source = ParserInput::new(value);
    let mut input = Parser::new(&mut source);
    let resolved = atom(&mut input, font, rem, percentages, 0)?;
    if !input.is_exhausted()
        || !resolved.0.is_finite()
        || resolved.0.abs() > 10000.
        || !resolved.1.is_finite()
        || resolved.1.abs() > 10000.
        || (resolved.2 != 1
            && !(resolved.0 == 0. && resolved.1 == 0. && value.trim().parse::<f64>() == Ok(0.)))
    {
        return None;
    }
    Some((resolved.0 as f32, resolved.1 as f32))
}

/// Split CSS component values without splitting whitespace or commas inside functions.
pub fn component_values(value: &str) -> Option<Vec<String>> {
    if value.len() > 65_536 {
        return None;
    }
    let mut source = ParserInput::new(value);
    let mut input = Parser::new(&mut source);
    let mut values = Vec::new();
    while !input.is_exhausted() {
        let start = input.position();
        let token = input.next().ok()?.clone();
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            input
                .parse_nested_block(|input| Ok::<_, ParseError<'_, ()>>(remainder(input)))
                .ok()?;
        }
        values.push(input.slice_from(start).trim().to_owned());
        if values.len() > 256 {
            return None;
        }
    }
    Some(values)
}
#[cfg(test)]
mod variable_tests {
    use super::*;
    #[test]
    fn relative_length_tokens_preserve_functions_strings_and_percentage_bases() {
        assert_eq!(
            font_relative_lengths("calc(1em + .5rem) 25% url(1em) \"2em\"", 12., 16.).as_deref(),
            Some("calc(12px + 8px) 25% url(1em) \"2em\"")
        );
        assert_eq!(
            font_relative_lengths("drop-shadow(-.5EM 1rem .25em red)", 20., 16.).as_deref(),
            Some("drop-shadow(-10px 16px 5px red)")
        );
        assert!(font_relative_lengths("1em", f32::NAN, 16.).is_none());
        let viewport = MediaEnvironment {
            width: 800.,
            height: 600.,
            ..Default::default()
        };
        assert_eq!(
            viewport_lengths("calc(100vh - 2em) 50% url(2vw) \"3vh\"", viewport).as_deref(),
            Some("calc(600px - 2em) 50% url(2vw) \"3vh\"")
        );
        assert_eq!(
            viewport_lengths("10vw 10vmin 10vmax 10SVH 10lvh 10dvh", viewport).as_deref(),
            Some("80px 60px 80px 60px 60px 60px")
        );
        assert!(
            viewport_lengths(
                "1vh",
                MediaEnvironment {
                    height: f32::NAN,
                    ..viewport
                }
            )
            .is_none()
        );
    }
    #[test]
    fn component_values_preserve_nested_functions_and_separate_shadow_layers() {
        assert_eq!(
            component_values("0 4px 12px rgb(2 3 4 / 20%), -1px 0 color(display-p3 1 0 0)"),
            Some(
                [
                    "0",
                    "4px",
                    "12px",
                    "rgb(2 3 4 / 20%)",
                    ",",
                    "-1px",
                    "0",
                    "color(display-p3 1 0 0)"
                ]
                .into_iter()
                .map(str::to_owned)
                .collect()
            )
        );
        assert!(component_values(&"1 ".repeat(257)).is_none());
    }
    #[test]
    fn variables_preserve_nested_colors_fallbacks_case_and_bounded_cycles() {
        let vars = BTreeMap::from([
            ("--A".into(), "red".into()),
            ("--a".into(), "blue".into()),
            (
                "--nested".into(),
                "rgb(var(--channels) / var(--alpha, 40%))".into(),
            ),
            ("--channels".into(), "1 2 3".into()),
            ("--loop".into(), "var(--loop)".into()),
        ]);
        assert_eq!(variables("var(--A)", &vars).as_deref(), Some("red"));
        assert_eq!(variables("var(--a)", &vars).as_deref(), Some("blue"));
        assert_eq!(
            variables("var(--nested)", &vars).as_deref(),
            Some("rgb(1 2 3 / 40%)")
        );
        assert_eq!(
            variables("linear-gradient(var(--missing, var(--A)), var(--a))", &vars).as_deref(),
            Some("linear-gradient(red, blue)")
        );
        assert!(variables("var(--loop)", &vars).is_none());
        assert_eq!(
            variables("var(--loop, green)", &vars).as_deref(),
            Some("green")
        );
        assert!(variables(&"a".repeat(65_537), &vars).is_none());
    }
}
