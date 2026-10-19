//! Original UTF-16 token alignment, independent of the Japanese tokenizer.
use crate::lyrics::Word;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub start_char: usize,
    pub end_char: usize,
    pub text: String,
    pub romaji: String,
    pub furigana: String,
}
fn length(text: &str) -> usize {
    text.encode_utf16().count()
}
fn slice(text: &str, start: usize, end: usize) -> String {
    String::from_utf16_lossy(
        &text
            .encode_utf16()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect::<Vec<_>>(),
    )
}
fn proportional(text: &str, start: usize, end: usize, total: usize) -> String {
    let count = length(text);
    if total == 0 || end <= start || count == 0 {
        return String::new();
    }
    let from = start * count / total;
    let to = if end >= total {
        count
    } else {
        end * count / total
    };
    slice(text, from, to.max((from + 1).min(count)))
}
fn ranges(text: &str, words: &[Word]) -> Option<Vec<(usize, usize)>> {
    let mut cursor = 0;
    let ranges = words
        .iter()
        .map(|word| {
            let start = cursor;
            cursor += length(&word.text);
            (start, cursor)
        })
        .collect();
    (cursor == length(text)).then_some(ranges)
}
pub fn romaji(text: &str, words: &[Word], tokens: &[Token]) -> Option<Vec<Word>> {
    let ranges = ranges(text, words)?;
    let mut result: Vec<_> = words
        .iter()
        .zip(&ranges)
        .map(|(word, &(start, end))| Word {
            text: tokens
                .iter()
                .filter(|token| start < token.end_char && end > token.start_char)
                .map(|token| {
                    proportional(
                        &token.romaji,
                        start.max(token.start_char) - token.start_char,
                        end.min(token.end_char) - token.start_char,
                        token.end_char.saturating_sub(token.start_char),
                    )
                })
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            ..word.clone()
        })
        .collect();
    static WHITESPACE: std::sync::OnceLock<regress::Regex> = std::sync::OnceLock::new();
    let whitespace = WHITESPACE.get_or_init(|| regress::Regex::with_flags(r"^\s+$", "u").unwrap());
    for pair in tokens.windows(2) {
        if whitespace.find(&pair[1].text).is_some() {
            continue;
        }
        if let Some(index) = ranges
            .iter()
            .enumerate()
            .filter(|(_, range)| {
                range.0 < pair[0].end_char
                    && range.1 > pair[0].start_char
                    && range.1 <= pair[0].end_char
            })
            .max_by_key(|(_, range)| range.1)
            .map(|(index, _)| index)
            && !result[index].text.is_empty()
            && !result[index].text.ends_with(' ')
        {
            result[index].text.push(' ')
        }
    }
    Some(result)
}
fn wrap(base: &str, reading: &str) -> String {
    format!("<ruby>{base}<rp>(</rp><rt>{reading}</rt><rp>)</rp></ruby>")
}
fn kanji(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}
fn characters(token: &Token) -> Vec<String> {
    let parsed = crate::lyrics_markup::parse(&token.furigana);
    let mut segments = vec![];
    for span in &parsed {
        if let Some(reading) = &span.ruby {
            let size = length(&span.text);
            if size == 1 {
                segments.push(wrap(&span.text, reading))
            } else {
                for (index, unit) in span.text.encode_utf16().enumerate() {
                    let base = String::from_utf16_lossy(&[unit]);
                    segments.push(if base.chars().any(kanji) {
                        wrap(&base, &proportional(reading, index, index + 1, size))
                    } else {
                        base
                    });
                }
            }
        } else {
            segments.extend(span.text.chars().map(|c| c.to_string()))
        }
    }
    if segments.len() == length(&token.text) {
        return segments;
    }
    token
        .text
        .chars()
        .enumerate()
        .map(|(index, c)| {
            if kanji(c)
                && parsed.len() == 1
                && parsed[0].text == token.text
                && let Some(reading) = &parsed[0].ruby
            {
                return wrap(
                    &c.to_string(),
                    &proportional(reading, index, index + 1, length(&token.text)),
                );
            }
            c.to_string()
        })
        .collect()
}
pub fn furigana(text: &str, words: &[Word], tokens: &[Token]) -> Option<Vec<Word>> {
    let ranges = ranges(text, words)?;
    Some(
        words
            .iter()
            .zip(ranges)
            .map(|(word, (start, end))| {
                let converted = tokens
                    .iter()
                    .filter(|token| start < token.end_char && end > token.start_char)
                    .map(|token| {
                        let from = start.max(token.start_char) - token.start_char;
                        let to = end.min(token.end_char) - token.start_char;
                        if from == 0 && to == token.end_char.saturating_sub(token.start_char) {
                            token.furigana.clone()
                        } else {
                            characters(token)
                                .into_iter()
                                .skip(from)
                                .take(to.saturating_sub(from))
                                .collect::<String>()
                        }
                    })
                    .collect::<String>();
                Word {
                    text: if converted.is_empty() {
                        word.text.clone()
                    } else {
                        converted
                    },
                    ..word.clone()
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn word_alignment_matches_original_pronunciation_and_ruby_helpers() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyric-conversion-reference.json"))
                .unwrap();
        fn words(value: &serde_json::Value) -> Option<Vec<crate::lyrics::Word>> {
            value.as_array().map(|words| {
                words
                    .iter()
                    .map(|word| crate::lyrics::Word {
                        time: word["startMs"].as_f64().unwrap() / 1000.,
                        end: Some(word["endMs"].as_f64().unwrap() / 1000.),
                        text: word["text"].as_str().unwrap().into(),
                    })
                    .collect()
            })
        }
        for case in fixture["cases"].as_array().unwrap() {
            let tokens: Vec<super::Token> = serde_json::from_value(case["tokens"].clone()).unwrap();
            let source = words(&case["words"]).unwrap();
            let text = case["text"].as_str().unwrap();
            assert_eq!(
                super::romaji(text, &source, &tokens),
                words(&case["romaji"]),
                "{case}"
            );
            assert_eq!(
                super::furigana(text, &source, &tokens),
                words(&case["furigana"]),
                "{case}"
            );
        }
    }
}
