use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

const MAX_BYTES: usize = 32_000_000;

include!(concat!(env!("OUT_DIR"), "/builtin_fonts.rs"));

pub fn builtin_fonts() -> Result<Vec<Data>> {
    BUILTIN_FONT_BYTES
        .iter()
        .map(|bytes| decode(bytes))
        .collect()
}

/// CSS font-style keywords and global resets, excluding angular declarations.
pub fn style_keyword(value: &str) -> Option<&'static str> {
    if value.len() > 4096 {
        return None;
    }
    let mut source = cssparser::ParserInput::new(value);
    let mut input = cssparser::Parser::new(&mut source);
    let cssparser::Token::Ident(value) = input.next().ok()? else {
        return None;
    };
    let keyword = match value.to_ascii_lowercase().as_str() {
        "normal" => "normal",
        "italic" => "italic",
        "oblique" => "oblique",
        "initial" => "initial",
        "inherit" => "inherit",
        "unset" => "unset",
        "revert" => "revert",
        _ => return None,
    };
    input.is_exhausted().then_some(keyword)
}

pub fn family_list(value: &str) -> Option<Vec<String>> {
    use cssparser::{ParseError, Token};
    if value.len() > 4096 {
        return None;
    }
    let mut source = cssparser::ParserInput::new(value);
    let mut input = cssparser::Parser::new(&mut source);
    let families = input
        .parse_comma_separated(|input| {
            let family = match input.next()?.clone() {
                Token::QuotedString(name) => name.to_string(),
                Token::Ident(name) => {
                    let mut names = vec![name.to_string()];
                    while !input.is_exhausted() {
                        names.push(input.expect_ident()?.to_string());
                    }
                    let first = names[0].to_ascii_lowercase();
                    if (names.len() == 1
                        && matches!(
                            first.as_str(),
                            "inherit" | "initial" | "unset" | "revert" | "revert-layer" | "default"
                        ))
                        || (names.len() > 1
                            && matches!(
                                first.as_str(),
                                "serif"
                                    | "sans-serif"
                                    | "monospace"
                                    | "cursive"
                                    | "fantasy"
                                    | "system-ui"
                                    | "ui-serif"
                                    | "ui-sans-serif"
                                    | "ui-monospace"
                                    | "ui-rounded"
                                    | "emoji"
                                    | "math"
                                    | "fangsong"
                            ))
                    {
                        return Err(input.new_custom_error(()));
                    }
                    names.join(" ")
                }
                _ => return Err(input.new_custom_error(())),
            };
            if family.contains('\0') || family.len() > 256 {
                return Err(input.new_custom_error(()));
            }
            Ok::<_, ParseError<'_, ()>>(family)
        })
        .ok()?;
    (families.len() <= 32).then_some(families)
}

pub fn numeric_features(value: &str) -> Option<Vec<(String, u32)>> {
    if value.len() > 4096 {
        return None;
    }
    let mut source = cssparser::ParserInput::new(value);
    let mut input = cssparser::Parser::new(&mut source);
    let mut groups = [None; 5];
    while !input.is_exhausted() {
        let name = input.expect_ident().ok()?.to_ascii_lowercase();
        if name == "normal" && groups.iter().all(Option::is_none) && input.is_exhausted() {
            return Some(vec![]);
        }
        let (group, tag) = match name.as_str() {
            "lining-nums" => (0, "lnum"),
            "oldstyle-nums" => (0, "onum"),
            "proportional-nums" => (1, "pnum"),
            "tabular-nums" => (1, "tnum"),
            "diagonal-fractions" => (2, "frac"),
            "stacked-fractions" => (2, "afrc"),
            "ordinal" => (3, "ordn"),
            "slashed-zero" => (4, "zero"),
            _ => return None,
        };
        if groups[group].replace(tag).is_some() {
            return None;
        }
    }
    groups.iter().any(Option::is_some).then(|| {
        groups
            .into_iter()
            .flatten()
            .map(|tag| (tag.into(), 1))
            .collect()
    })
}

pub fn variation_settings(value: &str) -> Option<Vec<(String, f32)>> {
    use cssparser::ParseError;
    if value.len() > 65_536 {
        return None;
    }
    let mut source = cssparser::ParserInput::new(value);
    let mut input = cssparser::Parser::new(&mut source);
    if input
        .try_parse(|input| input.expect_ident_matching("normal"))
        .is_ok()
    {
        return input.is_exhausted().then(Vec::new);
    }
    let values = input
        .parse_comma_separated(|input| {
            let tag = input.expect_string()?.to_string();
            let value = input.expect_number()?;
            if tag.len() != 4
                || !tag.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
                || !value.is_finite()
            {
                return Err(input.new_custom_error(()));
            }
            Ok::<_, ParseError<'_, ()>>((tag, if value == 0. { 0. } else { value }))
        })
        .ok()?;
    if values.len() > 64 {
        return None;
    }
    Some(
        values
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_iter()
            .collect(),
    )
}

pub fn feature_settings(value: &str) -> Option<Vec<(String, u32)>> {
    use cssparser::{ParseError, Token};
    if value.len() > 65_536 {
        return None;
    }
    let mut source = cssparser::ParserInput::new(value);
    let mut input = cssparser::Parser::new(&mut source);
    if input
        .try_parse(|input| input.expect_ident_matching("normal"))
        .is_ok()
        && input.is_exhausted()
    {
        return Some(vec![]);
    }
    let mut source = cssparser::ParserInput::new(value);
    let mut input = cssparser::Parser::new(&mut source);
    let features = input
        .parse_comma_separated(|input| {
            let tag = input.expect_string()?.to_string();
            if tag.len() != 4 || !tag.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
                return Err(input.new_custom_error(()));
            }
            let value = if input.is_exhausted() {
                1
            } else {
                match input.next()? {
                    Token::Number {
                        int_value: Some(value),
                        ..
                    } => {
                        // Chromium accepts signed values and passes their uint32 bit pattern to shaping.
                        *value as u32
                    }
                    Token::Ident(value) if value.eq_ignore_ascii_case("on") => 1,
                    Token::Ident(value) if value.eq_ignore_ascii_case("off") => 0,
                    _ => return Err(input.new_custom_error(())),
                }
            };
            Ok::<_, ParseError<'_, ()>>((tag, value))
        })
        .ok()?;
    if features.len() > 64 {
        return None;
    }
    Some(
        features
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_iter()
            .collect(),
    )
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    #[serde(rename = "type")]
    pub kind: String,
    pub built_in: String,
    pub custom: Option<String>,
    pub system: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            kind: "builtIn".into(),
            built_in: "Inter".into(),
            custom: None,
            system: None,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.kind.as_str(), "builtIn" | "system" | "custom"),
            "Invalid font type"
        );
        ensure!(
            matches!(self.built_in.as_str(), "Inter" | "Poppins"),
            "Invalid built-in font"
        );
        ensure!(
            self.custom
                .as_ref()
                .is_none_or(|path| !path.contains('\0') && path.len() <= 4096),
            "Invalid font path"
        );
        ensure!(
            self.system
                .as_ref()
                .is_none_or(|name| !name.contains('\0') && name.len() <= 256),
            "Invalid font family"
        );
        Ok(())
    }
}
pub struct Data {
    pub bytes: Vec<u8>,
    pub family: String,
}
fn bounded(
    reader: impl Read,
    expected: usize,
) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
    if expected > MAX_BYTES {
        return Err("Font expands beyond 32 MB".into());
    }
    let mut bytes = Vec::new();
    reader.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() != expected {
        return Err("Invalid expanded font length".into());
    }
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<Data> {
    ensure!(bytes.len() <= MAX_BYTES, "Font is larger than 32 MB");
    let signature = bytes.get(..4).context("Font header is missing")?;
    let bytes = if matches!(signature, b"wOFF" | b"wOF2") {
        let size = u32::from_be_bytes(
            bytes
                .get(16..20)
                .context("Font header is truncated")?
                .try_into()?,
        ) as usize;
        ensure!(size > 0 && size <= MAX_BYTES, "Font expands beyond 32 MB");
        if signature == b"wOFF" {
            let count = u16::from_be_bytes(
                bytes
                    .get(12..14)
                    .context("Font header is truncated")?
                    .try_into()?,
            ) as usize;
            let tables = bytes
                .get(44..44 + count * 20)
                .context("Font directory is truncated")?;
            let total = tables
                .as_chunks::<20>()
                .0
                .iter()
                .try_fold(12usize + count * 16, |total, table| {
                    total.checked_add(
                        (u32::from_be_bytes(table[12..16].try_into().unwrap()) as usize)
                            .checked_add(3)?
                            & !3,
                    )
                })
                .context("Font table lengths overflow")?;
            ensure!(
                total <= MAX_BYTES && total == size,
                "Invalid expanded font directory"
            );
            wuff::decompress_woff1_with_custom_z(bytes, &mut |compressed, expected| {
                bounded(flate2::read::ZlibDecoder::new(compressed), expected)
            })
            .map_err(|error| anyhow::anyhow!("Invalid WOFF font: {error}"))?
        } else {
            wuff::decompress_woff2_with_custom_brotli(bytes, &mut |compressed, expected| {
                bounded(
                    brotli_decompressor::Decompressor::new(compressed, 4096),
                    expected,
                )
            })
            .map_err(|error| anyhow::anyhow!("Invalid WOFF2 font: {error}"))?
        }
    } else {
        bytes.to_vec()
    };
    ensure!(bytes.len() <= MAX_BYTES, "Font expands beyond 32 MB");
    let face = ttf_parser::Face::parse(&bytes, 0).context("Invalid OpenType font")?;
    let family = [
        ttf_parser::name_id::TYPOGRAPHIC_FAMILY,
        ttf_parser::name_id::FAMILY,
    ]
    .into_iter()
    .find_map(|id| {
        face.names()
            .into_iter()
            .filter(|name| name.name_id == id)
            .find_map(|name| name.to_string())
    })
    .context("Font has no readable family name")?;
    ensure!(
        !family.trim().is_empty() && family.len() <= 256,
        "Invalid font family name"
    );
    Ok(Data { bytes, family })
}
pub fn read(path: &Path) -> Result<Data> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    decode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn predecoded_builtins_match_original_fonts() {
        assert_eq!(BUILTIN_FONT_BYTES.len(), BUILTIN_FONT_SOURCES.len());
        for (bytes, source) in BUILTIN_FONT_BYTES.iter().zip(BUILTIN_FONT_SOURCES) {
            let original = decode(source).unwrap();
            assert_eq!(*bytes, original.bytes, "{}", original.family);
            assert_eq!(decode(bytes).unwrap().family, original.family);
        }
    }
    #[test]
    fn css_font_stacks_and_embedded_fallback_faces_keep_the_original_mapping() {
        assert_eq!(
            family_list(r#"Inter, 'Noto Sans JP', Noto Sans Hebrew, sans-serif"#),
            Some(vec![
                "Inter".into(),
                "Noto Sans JP".into(),
                "Noto Sans Hebrew".into(),
                "sans-serif".into()
            ])
        );
        assert_eq!(
            family_list(r#""Name, with comma", 'inherit', Noto\20 Sans\20 JP"#),
            Some(vec![
                "Name, with comma".into(),
                "inherit".into(),
                "Noto Sans JP".into()
            ])
        );
        for value in [
            "",
            "inherit",
            "default",
            "Inter,",
            "Inter, 123",
            "Inter !important",
        ] {
            assert!(family_list(value).is_none(), "{value}");
        }
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../feishin-gpui/assets/font-fallbacks-reference.json"
        ))
        .unwrap();
        for case in reference["parsers"].as_array().unwrap() {
            let parsed = family_list(case["value"].as_str().unwrap());
            assert_eq!(parsed.is_some(), case["valid"].as_bool().unwrap(), "{case}");
            if let Some(parsed) = parsed {
                assert_eq!(
                    Some(parsed),
                    family_list(case["computed"].as_str().unwrap()),
                    "{case}"
                );
            }
        }
        let fonts = builtin_fonts().unwrap();
        assert_eq!(fonts.len(), 20);
        for family in ["Noto Sans JP", "Noto Sans Hebrew"] {
            let weights = fonts
                .iter()
                .filter(|font| font.family == family)
                .map(|font| {
                    let face = ttf_parser::Face::parse(&font.bytes, 0).unwrap();
                    assert!(!face.is_variable());
                    assert!(face.glyph_index('m').is_some());
                    assert!(
                        face.glyph_index(if family == "Noto Sans JP" {
                            '日'
                        } else {
                            'ש'
                        })
                        .is_some()
                    );
                    assert!(
                        face.glyph_index(if family == "Noto Sans JP" {
                            'ש'
                        } else {
                            '日'
                        })
                        .is_none()
                    );
                    face.weight().to_number()
                })
                .collect::<Vec<_>>();
            assert_eq!(weights, vec![400, 500, 600, 700, 800]);
            let variable = fonts
                .iter()
                .find(|font| font.family == format!("{family} Variable"))
                .unwrap();
            let face = ttf_parser::Face::parse(&variable.bytes, 0).unwrap();
            assert!(face.is_variable());
            assert!(
                face.variation_axes()
                    .into_iter()
                    .any(|axis| axis.tag == ttf_parser::Tag::from_bytes(b"wght"))
            );
            assert!(
                face.glyph_index(if family == "Noto Sans JP" {
                    '日'
                } else {
                    'ש'
                })
                .is_some()
            );
            assert!(
                face.glyph_index(if family == "Noto Sans JP" {
                    'ש'
                } else {
                    '日'
                })
                .is_none()
            );
        }
        let symbols = fonts.iter().find(|font| font.family == "Symbola").unwrap();
        let face = ttf_parser::Face::parse(&symbols.bytes, 0).unwrap();
        assert!(face.glyph_index('🅴').is_some() && face.glyph_index('🅲').is_some());
    }
    #[test]
    fn opentype_css_matches_chromium_validity_and_computed_features() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../feishin-gpui/assets/font-features-reference.json"
        ))
        .unwrap();
        for case in reference["parsers"].as_array().unwrap() {
            let property = case["property"].as_str().unwrap();
            let value = case["value"].as_str().unwrap();
            let parsed = if property == "font-variant-numeric" {
                numeric_features(value)
            } else {
                feature_settings(value)
            };
            assert_eq!(parsed.is_some(), case["valid"].as_bool().unwrap(), "{case}");
            if let Some(parsed) = parsed {
                let computed = case["computed"].as_str().unwrap();
                if computed.contains("e+") {
                    // Chromium's large-integer serialization loses precision and is not valid CSS.
                    assert_eq!(parsed, vec![("tnum".into(), i32::MAX as u32)]);
                    continue;
                }
                let expected = if property == "font-variant-numeric" {
                    numeric_features(computed)
                } else {
                    feature_settings(computed)
                };
                assert_eq!(Some(parsed), expected, "{case}");
            }
        }
        assert!(numeric_features(&" ".repeat(4097)).is_none());
        assert!(feature_settings(&" ".repeat(65_537)).is_none());
        assert!(feature_settings(&vec!["\"abcd\""; 65].join(",")).is_none());
    }
    #[test]
    fn existing_web_fonts_decode_without_changing_families_and_reject_expansion_limits() {
        for (source, expected) in [
            (
                include_bytes!("../../../assets/fonts/Inter-VariableFont_opsz,wght.woff2")
                    .as_slice(),
                "Inter",
            ),
            (
                include_bytes!("../../../assets/fonts/Poppins-Regular.woff2").as_slice(),
                "Poppins",
            ),
        ] {
            let decoded = decode(source).unwrap();
            assert_eq!(decoded.family, expected);
            assert_eq!(decode(&decoded.bytes).unwrap().family, expected);
            let mut bomb = source.to_vec();
            bomb[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
            assert!(decode(&bomb).is_err());
            assert!(decode(&source[..50]).is_err());
        }
        assert!(decode(b"bad font").is_err());
        let config = Config {
            kind: "unsupported".into(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
