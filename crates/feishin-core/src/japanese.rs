//! Offline Japanese lyric readings using the same IPADIC family as the original app.
use crate::lyrics::Line;
use anyhow::{Result, ensure};
use lindera::{dictionary::load_dictionary, mode::Mode, segmenter::Segmenter};
use std::{borrow::Cow, sync::OnceLock};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub ruby: String,
    pub time: Option<f64>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Annotation {
    pub spans: Vec<Span>,
    pub romaji: Vec<Span>,
    pub cues: Vec<Annotation>,
    pub tokens: Vec<crate::lyric_conversion::Token>,
}

/// Keep readings in the same timed word so the text and ruby share a highlight.
pub fn furigana_words(
    annotation: &Annotation,
    source: &[crate::lyrics::Word],
) -> Vec<crate::lyrics::Word> {
    if !source.is_empty() {
        let text = annotation
            .tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect::<String>();
        return crate::lyric_conversion::furigana(&text, source, &annotation.tokens)
            .unwrap_or_else(|| source.to_vec());
    }
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut words: Vec<crate::lyrics::Word> = vec![];
    for span in &annotation.spans {
        let time = span.time.unwrap_or_default();
        let text = if span.ruby.is_empty() {
            escape(&span.text)
        } else {
            format!(
                "<ruby>{}<rt>{}</rt></ruby>",
                escape(&span.text),
                escape(&span.ruby)
            )
        };
        if let Some(word) = words.last_mut().filter(|word| word.time == time) {
            word.text.push_str(&text);
        } else {
            words.push(crate::lyrics::Word {
                time,
                text,
                end: source
                    .iter()
                    .find(|word| word.time == time)
                    .and_then(|word| word.end),
            });
        }
    }
    words
}
pub fn romaji_words(
    annotation: &Annotation,
    source: &[crate::lyrics::Word],
) -> Option<Vec<crate::lyrics::Word>> {
    if annotation.tokens.is_empty() || source.is_empty() {
        return None;
    }
    let text = annotation
        .tokens
        .iter()
        .map(|token| token.text.as_str())
        .collect::<String>();
    crate::lyric_conversion::romaji(&text, source, &annotation.tokens)
}
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn token(
    text: &str,
    romaji: String,
    furigana: String,
    prefix: &str,
) -> crate::lyric_conversion::Token {
    let start = prefix.encode_utf16().count();
    crate::lyric_conversion::Token {
        start_char: start,
        end_char: start + text.encode_utf16().count(),
        text: text.into(),
        romaji,
        furigana,
    }
}
fn kana(c: char) -> bool {
    ('\u{3040}'..='\u{309f}').contains(&c) || ('\u{30a0}'..='\u{30ff}').contains(&c)
}
fn kanji(c: char) -> bool {
    ('\u{3400}'..='\u{4dbf}').contains(&c)
        || ('\u{4e00}'..='\u{9fcf}').contains(&c)
        || ('\u{f900}'..='\u{faff}').contains(&c)
}
fn timing(line: &Line, byte: usize) -> Option<f64> {
    let mut end = 0;
    for word in &line.words {
        end += word.text.len();
        if byte < end {
            return Some(word.time);
        }
    }
    line.time
}
struct Morph {
    surface: String,
    reading: String,
    pronounced: String,
    pos: String,
    start: usize,
    end: usize,
}
fn analyze(segmenter: &Segmenter, text: &str) -> Result<Vec<Morph>> {
    let mut tokens = segmenter
        .segment(Cow::Borrowed(text))?
        .into_iter()
        .map(|mut token| {
            let surface = token.surface.to_string();
            let reading = crate::hepburn::hiragana(
                token
                    .get_detail(7)
                    .filter(|text| *text != "*")
                    .unwrap_or(&surface),
            );
            let pronounced = token
                .get_detail(8)
                .filter(|text| *text != "*")
                .unwrap_or(&reading)
                .to_string();
            let pos = token.get_detail(0).unwrap_or_default().to_string();
            Morph {
                surface,
                reading,
                pronounced,
                pos,
                start: token.byte_start,
                end: token.byte_end,
            }
        })
        .collect::<Vec<_>>();
    let mut index = 1;
    while index < tokens.len() {
        if tokens[index].pos == "助動詞"
            && matches!(tokens[index].surface.as_str(), "う" | "ウ")
            && tokens[index - 1].pos == "動詞"
        {
            let next = tokens.remove(index);
            let previous = &mut tokens[index - 1];
            previous.surface.push('う');
            previous.reading.push('う');
            previous.pronounced.push('ー');
            previous.end = next.end;
        } else {
            index += 1
        }
    }
    let mut index = 0;
    while index + 1 < tokens.len() {
        if matches!(tokens[index].pos.as_str(), "動詞" | "形容詞")
            && tokens[index].surface.encode_utf16().count() > 1
            && tokens[index].surface.ends_with(['っ', 'ッ'])
        {
            let next = tokens.remove(index + 1);
            let previous = &mut tokens[index];
            previous.surface.push_str(&next.surface);
            previous.reading.push_str(&next.reading);
            previous.pronounced.push_str(&next.pronounced);
            previous.end = next.end;
        } else {
            index += 1
        }
    }
    Ok(tokens)
}
fn ruby(surface: &str, reading: &str) -> Vec<(String, String)> {
    if !surface.chars().any(kanji) || reading.is_empty() || reading == "*" {
        return vec![(surface.into(), String::new())];
    }
    if !surface.chars().any(kana) {
        return vec![(surface.into(), reading.into())];
    }
    let mut pattern = String::from("^");
    let mut parts: Vec<(String, bool)> = vec![];
    for c in surface.chars() {
        if kanji(c) {
            if let Some((text, true)) = parts.last_mut() {
                text.push(c)
            } else {
                parts.push((c.to_string(), true));
                pattern.push_str("(.+)");
            }
        } else {
            parts.push((c.to_string(), false));
            pattern.push_str(&crate::hepburn::hiragana(&c.to_string()));
        }
    }
    pattern.push('$');
    let Ok(regex) = regex::Regex::new(&pattern) else {
        return vec![(surface.into(), String::new())];
    };
    let Some(captures) = regex.captures(reading) else {
        return vec![(surface.into(), String::new())];
    };
    let mut group = 0;
    parts
        .into_iter()
        .map(|(text, is_kanji)| {
            let ruby = if is_kanji {
                group += 1;
                captures[group].into()
            } else {
                String::new()
            };
            (text, ruby)
        })
        .collect()
}
fn fragment(segmenter: &Segmenter, text: &str) -> Result<String> {
    if !text.chars().any(|c| kana(c) || kanji(c)) {
        return Ok(escaped(text));
    }
    let mut result = String::new();
    let mut cursor = 0;
    for token in analyze(segmenter, text)? {
        result.push_str(&escaped(&text[cursor..token.start]));
        for (text, reading) in ruby(&token.surface, &token.reading) {
            if reading.is_empty() {
                result.push_str(&escaped(&text))
            } else {
                result.push_str(&format!(
                    "<ruby>{}<rp>(</rp><rt>{}</rt><rp>)</rp></ruby>",
                    escaped(&text),
                    escaped(&reading)
                ))
            }
        }
        cursor = token.end;
    }
    result.push_str(&escaped(&text[cursor..]));
    Ok(result)
}

pub fn annotate(lines: &[Line]) -> Result<Vec<Annotation>> {
    ensure!(
        lines.len() <= 10_000 && lines.iter().all(|line| line.text.len() <= 65_536),
        "Lyrics are too large to annotate"
    );
    // Matching the original, kana distinguishes Japanese lyrics from Chinese text.
    if !lines.iter().any(|line| {
        line.text.chars().any(kana) || line.cue_lines.iter().any(|cue| cue.text.chars().any(kana))
    }) {
        return Ok(vec![Annotation::default(); lines.len()]);
    }
    static SEGMENTER: OnceLock<Result<Segmenter, String>> = OnceLock::new();
    let segmenter = SEGMENTER
        .get_or_init(|| {
            load_dictionary("embedded://ipadic")
                .map(|dictionary| Segmenter::new(Mode::Normal, dictionary, None))
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|_| anyhow::anyhow!("The bundled Japanese dictionary could not be loaded"))?;
    lines
        .iter()
        .map(|line| {
            let mut result = Annotation::default();
            let mut cursor = 0;
            let has_kana = line.text.chars().any(kana);
            for token in analyze(segmenter, &line.text)? {
                if token.start > cursor {
                    let text = line.text[cursor..token.start].to_owned();
                    if has_kana {
                        result.tokens.push(self::token(
                            &text,
                            text.clone(),
                            escaped(&text),
                            &line.text[..cursor],
                        ));
                    }
                    let span = Span {
                        text,
                        time: timing(line, cursor),
                        ..Default::default()
                    };
                    result.spans.push(span.clone());
                    result.romaji.push(span);
                }
                let surface = token.surface;
                let reading = token.reading;
                let pronounced = token.pronounced;
                let mut start = token.start;
                let fragments = ruby(&surface, &reading);
                for (text, reading) in fragments {
                    let time = timing(line, start);
                    start += text.len();
                    result.spans.push(Span {
                        text,
                        ruby: reading,
                        time,
                    });
                }
                let romanized = if surface.chars().any(|c| kana(c) || kanji(c)) {
                    crate::hepburn::romanize(&pronounced)
                } else {
                    surface.clone()
                };
                if has_kana {
                    result.tokens.push(self::token(
                        &surface,
                        romanized.clone(),
                        fragment(segmenter, &surface)?,
                        &line.text[..token.start],
                    ));
                }
                result.romaji.push(Span {
                    text: crate::hepburn::romanize(&romanized),
                    time: timing(line, token.start),
                    ..Default::default()
                });
                cursor = token.end;
            }
            if cursor < line.text.len() {
                if has_kana {
                    let text = &line.text[cursor..];
                    result.tokens.push(self::token(
                        text,
                        text.into(),
                        escaped(text),
                        &line.text[..cursor],
                    ));
                }
                let span = Span {
                    text: line.text[cursor..].into(),
                    time: timing(line, cursor),
                    ..Default::default()
                };
                result.spans.push(span.clone());
                result.romaji.push(span);
            }
            if !has_kana {
                result.romaji.clear();
            }
            if !line.cue_lines.is_empty() {
                let cues: Vec<_> = line
                    .cue_lines
                    .iter()
                    .map(|cue| Line {
                        time: Some(cue.start),
                        text: cue.text.clone(),
                        words: cue.words.clone(),
                        cue_lines: Vec::new(),
                    })
                    .collect();
                result.cues = annotate(&cues)?;
            }
            Ok(result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn japanese_character_ranges_match_original_converter() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/japanese-reference.json")).unwrap();
        for case in reference["characters"].as_array().unwrap() {
            let c = case["text"].as_str().unwrap().chars().next().unwrap();
            assert_eq!(kana(c), case["kana"].as_bool().unwrap(), "{case}");
            assert_eq!(kanji(c), case["kanji"].as_bool().unwrap(), "{case}");
        }
    }
    #[test]
    fn richsync_annotations_keep_each_singer_text_and_word_clock() {
        let variant = crate::lyrics::structured_variant(
            &serde_json::json!({"synced":true,"line":[{"start":1000,"value":"Duet"}],"cueLine":[
                {"index":0,"start":1000,"value":"君の声","cue":[{"start":1000,"end":1500,"value":"君の"},{"start":1500,"end":2000,"value":"声"}]},
                {"index":0,"start":1200,"value":"取り戻す","cue":[{"start":1200,"end":2200,"value":"取り戻す"}]}
            ]}),
        );
        let result = annotate(&variant.lines).unwrap();
        assert_eq!(result[0].cues.len(), 2);
        for (cue, annotation) in variant.lines[0].cue_lines.iter().zip(&result[0].cues) {
            assert_eq!(
                annotation
                    .spans
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect::<String>(),
                cue.text
            );
            let words = furigana_words(annotation, &cue.words);
            assert_eq!(
                words.iter().map(|word| word.time).collect::<Vec<_>>(),
                cue.words.iter().map(|word| word.time).collect::<Vec<_>>()
            );
            assert_eq!(
                words.iter().map(|word| word.end).collect::<Vec<_>>(),
                cue.words.iter().map(|word| word.end).collect::<Vec<_>>()
            );
            assert_eq!(
                words
                    .iter()
                    .flat_map(|word| crate::lyrics_markup::parse(&word.text))
                    .map(|span| span.text)
                    .collect::<String>(),
                cue.text
            );
            let romaji = romaji_words(annotation, &cue.words).unwrap();
            assert_eq!(
                romaji
                    .iter()
                    .map(|word| (word.time, word.end))
                    .collect::<Vec<_>>(),
                cue.words
                    .iter()
                    .map(|word| (word.time, word.end))
                    .collect::<Vec<_>>()
            );
        }
        assert!(
            result[0].cues[0]
                .romaji
                .iter()
                .any(|span| span.text == "koe" && span.time == Some(1.5))
        );
        assert!(
            result[0].cues[1]
                .spans
                .iter()
                .any(|span| !span.ruby.is_empty() && span.time == Some(1.2))
        );
    }
    #[test]
    fn japanese_readings_preserve_original_text_and_synchronized_word_times_offline() {
        let lines = crate::lyrics::parse_lrc(
            "[00:01]<00:01>君の<00:02>声を聴く\n[00:04]取り戻す ABC 123 😀\n[00:08]English",
        );
        let result = annotate(&lines).unwrap();
        for (line, annotation) in lines.iter().zip(&result) {
            assert_eq!(
                annotation
                    .spans
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect::<String>(),
                line.text
            );
        }
        assert!(
            result[0]
                .spans
                .iter()
                .any(|span| span.text == "君" && span.ruby == "きみ")
        );
        assert!(
            result[0]
                .romaji
                .iter()
                .any(|span| span.text == "kimi" && span.time == Some(1.0))
        );
        assert!(
            result[0]
                .romaji
                .iter()
                .any(|span| span.text == "koe" && span.time == Some(2.0))
        );
        assert_eq!(
            ruby("取り戻す", "とりもどす"),
            vec![
                ("取".into(), "と".into()),
                ("り".into(), "".into()),
                ("戻".into(), "もど".into()),
                ("す".into(), "".into())
            ]
        );
        assert!(result[2].spans.iter().all(|span| span.ruby.is_empty()));
        assert!(
            annotate(&crate::lyrics::parse_lrc("中文歌词\nEnglish"))
                .unwrap()
                .iter()
                .all(|line| line.spans.is_empty())
        );
        assert!(
            annotate(&crate::lyrics::parse_lrc("ハロー世界")).unwrap()[0]
                .romaji
                .iter()
                .any(|span| span.text == "harō")
        );
    }

    #[test]
    fn pronunciation_splits_morphemes_at_original_word_boundaries() {
        let line = crate::lyrics::Line {
            time: Some(1.),
            text: "音楽の".into(),
            words: ["音", "楽", "の"]
                .into_iter()
                .enumerate()
                .map(|(index, text)| crate::lyrics::Word {
                    time: 1. + index as f64,
                    end: Some(2. + index as f64),
                    text: text.into(),
                })
                .collect(),
            cue_lines: vec![],
        };
        let annotation = annotate(std::slice::from_ref(&line)).unwrap().remove(0);
        let ruby = furigana_words(&annotation, &line.words);
        assert_eq!(ruby.len(), 3);
        for (word, base, reading) in [(&ruby[0], "音", "おん"), (&ruby[1], "楽", "がく")] {
            let spans = crate::lyrics_markup::parse(&word.text);
            assert_eq!(spans[0].text, base);
            assert_eq!(spans[0].ruby.as_deref(), Some(reading));
        }
        assert_eq!(
            romaji_words(&annotation, &line.words)
                .unwrap()
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>(),
            ["ong", "aku ", "no"]
        );
        assert_eq!(
            ruby.iter()
                .map(|word| (word.time, word.end))
                .collect::<Vec<_>>(),
            line.words
                .iter()
                .map(|word| (word.time, word.end))
                .collect::<Vec<_>>()
        );
    }
}
#[test]
fn japanese_tokens_match_original_offline_dictionary() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../assets/japanese-reference.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let line = crate::lyrics::Line {
            time: Some(1.),
            text: text.into(),
            words: vec![],
            cue_lines: vec![],
        };
        let actual = annotate(&[line]).unwrap().remove(0);
        let expected: Vec<crate::lyric_conversion::Token> =
            serde_json::from_value(case["tokens"].clone()).unwrap();
        assert_eq!(actual.tokens, expected, "{text}");
        let romanized = actual
            .romaji
            .iter()
            .map(|span| span.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            romanized,
            case["romaji"].as_str().unwrap(),
            "Plain romaji: {text}"
        );
        let actual_html = if actual.spans.is_empty() {
            text.to_string()
        } else {
            furigana_words(&actual, &[])
                .iter()
                .map(|word| word.text.as_str())
                .collect()
        };
        assert_eq!(
            crate::lyrics_markup::parse(&actual_html),
            crate::lyrics_markup::parse(case["furigana"].as_str().unwrap()),
            "Full-line furigana: {text}"
        );
    }
    for case in fixture["syllables"].as_array().unwrap() {
        assert_eq!(
            crate::hepburn::romanize(case["text"].as_str().unwrap()),
            case["romaji"].as_str().unwrap(),
            "{case}"
        );
    }
}
