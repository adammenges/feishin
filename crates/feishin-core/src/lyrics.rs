use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct Variant {
    pub language: String,
    pub kind: String,
    pub synchronized: bool,
    pub offset: Option<f64>,
    pub lines: Vec<Line>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<Agent>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default, PartialEq)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub role: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CueLine {
    pub start: f64,
    pub end: Option<f64>,
    pub text: String,
    pub agent_id: Option<String>,
    pub words: Vec<Word>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Display {
    pub font_size: f32,
    pub font_size_unsync: f32,
    pub gap: f32,
    pub gap_unsync: f32,
    pub opacity_non_active: f32,
    pub scale_non_active: f32,
    pub padding_left: f32,
    pub padding_right: f32,
}
impl Default for Display {
    fn default() -> Self {
        Self {
            font_size: 24.0,
            font_size_unsync: 24.0,
            gap: 24.0,
            gap_unsync: 24.0,
            opacity_non_active: 0.2,
            scale_non_active: 0.95,
            padding_left: 0.0,
            padding_right: 0.0,
        }
    }
}
impl Display {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (8.0..=128.0).contains(&self.font_size)
                && (8.0..=128.0).contains(&self.font_size_unsync)
                && (0.0..=128.0).contains(&self.gap)
                && (0.0..=128.0).contains(&self.gap_unsync)
                && (0.0..=1.0).contains(&self.opacity_non_active)
                && (0.1..=1.0).contains(&self.scale_non_active)
                && (0.0..=500.0).contains(&self.padding_left)
                && (0.0..=500.0).contains(&self.padding_right),
            "Invalid lyric display settings"
        );
        Ok(())
    }
}

/// Advance scrolling before a line starts, while keeping highlighting on its actual time.
pub fn follow_line(lines: &[Line], position: f64, lead_seconds: f64) -> Option<usize> {
    active_line(lines, position + lead_seconds)
        .or_else(|| lines.iter().position(|line| line.time.is_some()))
}
pub fn follow_ratio(alignment: f32) -> f32 {
    (0.5 + alignment / 100.0).clamp(0.05, 0.95)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Line {
    pub time: Option<f64>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<Word>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cue_lines: Vec<CueLine>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Word {
    pub time: f64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
}

fn timestamp(input: &str) -> Option<f64> {
    let (minutes, seconds) = input.rsplit_once(':')?;
    let seconds: f64 = seconds.parse().ok()?;
    let minutes = if let Some((hours, minutes)) = minutes.split_once(':') {
        let hours: u32 = hours.parse().ok()?;
        let minutes: u32 = minutes.parse().ok()?;
        if minutes >= 60 {
            return None;
        }
        hours as f64 * 60.0 + minutes as f64
    } else {
        minutes.parse::<u32>().ok()? as f64
    };
    (seconds.is_finite() && (0.0..60.0).contains(&seconds)).then_some(minutes * 60.0 + seconds)
}

fn words(input: &str) -> (String, Vec<Word>) {
    let mut remaining = input;
    let mut plain = String::new();
    let mut result = vec![];
    let mut time = None;
    while let Some(start) = remaining.find('<') {
        let Some(end) = remaining[start..].find('>') else {
            break;
        };
        let Some(next) = timestamp(&remaining[start + 1..start + end]) else {
            break;
        };
        let text = &remaining[..start];
        plain.push_str(text);
        if let Some(time) = time {
            result.push(Word {
                time,
                text: text.into(),
                end: None,
            });
        }
        time = Some(next);
        remaining = &remaining[start + end + 1..];
    }
    plain.push_str(remaining);
    if let Some(time) = time {
        result.push(Word {
            time,
            text: remaining.into(),
            end: None,
        });
    }
    (plain, result)
}

/// Retain the original raw-text formatter's grammar, priority and line order.
pub fn display_lines(input: &str) -> Vec<Line> {
    if input.is_empty() {
        return vec![];
    }
    static PATTERNS: std::sync::OnceLock<(regex::Regex, regex::Regex, regex::Regex)> =
        std::sync::OnceLock::new();
    let (standard, timing, cues) = PATTERNS.get_or_init(|| {
        (
            regex::Regex::new(r"\[([0-9]{2,}):([0-9]{2})(?:\.([0-9]{2,3}))?\]([^\n]+)(?:\n|$)")
                .unwrap(),
            regex::Regex::new(r"\[([0-9]*),[0-9]*\]([^\n]+)(?:\n|$)").unwrap(),
            regex::Regex::new(r"\([0-9]+,[0-9]+\)").unwrap(),
        )
    });
    let synchronized = standard
        .captures_iter(input)
        .filter_map(|capture| {
            let minutes = capture[1].parse::<f64>().ok()?;
            let seconds = capture[2].parse::<f64>().ok()?;
            // Whole-second tags keep finite timing rather than the source's absent-fraction NaN.
            let millis = capture.get(3).map_or(Some(0.), |fraction| {
                fraction.as_str().parse::<f64>().ok().map(|value| {
                    if fraction.as_str().len() == 3 {
                        value
                    } else {
                        value * 10.
                    }
                })
            })?;
            let time = minutes * 60. + seconds + millis / 1000.;
            time.is_finite().then(|| Line {
                time: Some(time),
                text: capture[4].into(),
                words: vec![],
                cue_lines: vec![],
            })
        })
        .collect::<Vec<_>>();
    if !synchronized.is_empty() {
        return synchronized;
    }
    let alternate = timing
        .captures_iter(input)
        .filter_map(|capture| {
            let time = if capture[1].is_empty() {
                0.
            } else {
                capture[1].parse::<f64>().ok()?
            };
            if !time.is_finite() {
                return None;
            }
            let body = cues.replace_all(&capture[2], "");
            let mut chars = body.chars().peekable();
            let mut text = String::new();
            while let Some(ch) = chars.next() {
                let whitespace = ch != '\u{0085}' && (ch.is_whitespace() || ch == '\u{feff}');
                if whitespace && chars.peek().is_some_and(|ch| matches!(ch, ',' | '.')) {
                    continue;
                }
                text.push(ch);
            }
            Some(Line {
                time: Some(time / 1000.),
                text,
                words: vec![],
                cue_lines: vec![],
            })
        })
        .collect::<Vec<_>>();
    if !alternate.is_empty() {
        return alternate;
    }
    input
        .split('\n')
        .map(|text| Line {
            time: None,
            text: text.into(),
            words: vec![],
            cue_lines: vec![],
        })
        .collect()
}

pub fn parse_lrc(input: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for raw in input.lines() {
        let mut text = raw.trim();
        let mut times = Vec::new();
        while let Some(rest) = text.strip_prefix('[') {
            let Some((tag, remaining)) = rest.split_once(']') else {
                break;
            };
            let Some(time) = timestamp(tag) else {
                break;
            };
            times.push(time);
            text = remaining;
        }
        if times.is_empty() {
            if !text.starts_with('[') && !text.is_empty() {
                lines.push(Line {
                    time: None,
                    text: text.into(),
                    words: vec![],
                    cue_lines: vec![],
                });
            }
        } else {
            for time in times {
                let (text, words) = words(text.trim());
                lines.push(Line {
                    time: Some(time),
                    text,
                    words,
                    cue_lines: vec![],
                });
            }
        }
    }
    // The original raw-LRC display ignores metadata offsets. Display offsets
    // come from structured metadata or the user's per-song/global preference.
    lines.sort_by(|a, b| a.time.unwrap_or(0.0).total_cmp(&b.time.unwrap_or(0.0)));
    lines
}

pub fn render_lrc(lines: &[Line]) -> String {
    fn tag(time: f64) -> String {
        let centiseconds = (time.max(0.0) * 100.0).round() as u64;
        format!(
            "{:02}:{:02}.{:02}",
            centiseconds / 6000,
            centiseconds / 100 % 60,
            centiseconds % 100
        )
    }
    lines
        .iter()
        .map(|line| {
            let text = if line.words.is_empty() {
                line.text.clone()
            } else {
                line.words
                    .iter()
                    .map(|word| format!("<{}>{}", tag(word.time), word.text))
                    .collect()
            };
            line.time
                .map_or_else(|| text.clone(), |time| format!("[{}]{text}", tag(time)))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn merge_translation(original: &str, translated: &str) -> String {
    let translations = parse_lrc(translated);
    let mut lines = parse_lrc(original);
    for line in &mut lines {
        if let Some(other) = translations.iter().find(|other| {
            line.time.is_some()
                && line
                    .time
                    .zip(other.time)
                    .is_some_and(|(a, b)| (a - b).abs() < 0.01)
        }) && !other.text.is_empty()
            && !line.text.is_empty()
        {
            line.text.push_str(&format!("\n{}", other.text));
            line.words.clear();
        }
    }
    // LRC lines cannot contain literal newlines. Preserve translations as a displayed separator.
    for line in &mut lines {
        line.text = line.text.replace('\n', " · ");
    }
    render_lrc(&lines)
}

/// OpenSubsonic sets can contain simultaneous singer lines and UTF-8 byte-indexed words.
pub fn structured_variant(value: &serde_json::Value) -> Variant {
    let synchronized = value["synced"].as_bool().unwrap_or(false);
    let cues = value["cueLine"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut lines: Vec<_> = value["line"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, line)| {
            let text = line["value"].as_str().unwrap_or_default().to_owned();
            let cue_lines: Vec<_> = cues
                .iter()
                .filter(|cue| cue["index"].as_u64() == Some(index as u64))
                .map(|cue_line| {
                    let cue_text = cue_line["value"].as_str().unwrap_or(&text);
                    let cues = cue_line["cue"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let mut words: Vec<Word> = cues
                        .iter()
                        .enumerate()
                        .filter_map(|(index, cue)| {
                            let time = milliseconds(&cue["start"])?;
                            let start = cue["byteStart"]
                                .as_u64()
                                .and_then(|n| usize::try_from(n).ok())
                                .unwrap_or(0);
                            let end = cue["byteEnd"]
                                .as_u64()
                                .and_then(|n| usize::try_from(n).ok())
                                .and_then(|n| n.checked_add(1))
                                .unwrap_or(start);
                            let mut text = cue["value"]
                                .as_str()
                                .filter(|text| !text.is_empty())
                                .unwrap_or_else(|| cue_text.get(start..end).unwrap_or_default())
                                .to_owned();
                            if let Some(next) = cues
                                .get(index + 1)
                                .and_then(|cue| cue["byteStart"].as_u64())
                                .and_then(|n| usize::try_from(n).ok())
                                && let Some(gap) = cue_text.get(end..next)
                            {
                                text.push_str(gap);
                            }
                            (!text.is_empty()).then(|| Word {
                                time,
                                text,
                                end: milliseconds(&cue["end"]).filter(|end| *end >= time),
                            })
                        })
                        .collect();
                    if words
                        .iter()
                        .map(|word| word.text.as_str())
                        .collect::<String>()
                        .trim()
                        != cue_text.trim()
                    {
                        words.clear();
                    }
                    let start = milliseconds(&cue_line["start"])
                        .or_else(|| milliseconds(&line["start"]))
                        .unwrap_or_default();
                    CueLine {
                        start,
                        end: milliseconds(&cue_line["end"]).filter(|end| *end >= start),
                        text: cue_text.to_owned(),
                        agent_id: cue_line["agentId"].as_str().map(str::to_owned),
                        words,
                    }
                })
                .collect();
            // A single complete cue also feeds the existing annotation and LRC paths.
            let words = cue_lines
                .first()
                .filter(|_| cue_lines.len() == 1)
                .filter(|cue| {
                    cue.words
                        .iter()
                        .map(|word| word.text.as_str())
                        .collect::<String>()
                        .trim()
                        == text.trim()
                })
                .map(|cue| cue.words.clone())
                .unwrap_or_default();
            Line {
                time: synchronized.then(|| milliseconds(&line["start"]).unwrap_or_default()),
                text,
                words,
                cue_lines,
            }
        })
        .collect();
    if synchronized {
        lines.sort_by(|a, b| a.time.unwrap_or(0.).total_cmp(&b.time.unwrap_or(0.)));
    }
    Variant {
        language: value["lang"].as_str().unwrap_or("und").into(),
        kind: if synchronized {
            value["kind"].as_str().unwrap_or("main")
        } else {
            "main"
        }
        .into(),
        synchronized,
        offset: value["offset"]
            .as_f64()
            .filter(|n| n.is_finite() && *n != 0. && n.abs() <= 86_400_000.)
            .map(|n| n / 1000.),
        lines,
        agents: value["agents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|agent| {
                Some(Agent {
                    id: agent["id"].as_str()?.to_owned(),
                    name: agent["name"].as_str().unwrap_or_default().to_owned(),
                    role: agent["role"].as_str().unwrap_or_default().to_owned(),
                })
            })
            .collect(),
    }
}

fn milliseconds(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|n| n.is_finite() && *n >= 0.)
        .map(|n| n / 1000.)
}

/// Match the original language-aware wrapping boundaries for long karaoke cues.
pub fn split_word_cues(words: &[Word], line_end: Option<f64>) -> Vec<Word> {
    use icu_segmenter::{
        GraphemeClusterSegmenter, WordSegmenter, options::WordBreakInvariantOptions,
    };
    use std::sync::OnceLock;
    static RUBY: OnceLock<regress::Regex> = OnceLock::new();
    let ruby = RUBY.get_or_init(|| regress::Regex::with_flags(r"<ruby[\s>]", "i").unwrap());
    if words.iter().any(|word| ruby.find(&word.text).is_some()) {
        return words.to_vec();
    }
    static BREAK: OnceLock<regress::Regex> = OnceLock::new();
    static GRAPHEME: OnceLock<regress::Regex> = OnceLock::new();
    let breaks = BREAK.get_or_init(|| {
        regress::Regex::with_flags(r"[\s\u200B\u00AD\p{Dash_Punctuation}]", "u").unwrap()
    });
    let grapheme = GRAPHEME.get_or_init(|| regress::Regex::with_flags(r"[\p{sc=Han}\p{sc=Hiragana}\p{sc=Katakana}\p{sc=Hangul}\p{sc=Thai}\p{sc=Lao}\p{sc=Khmer}\p{sc=Myanmar}]", "u").unwrap());
    let segmenter = WordSegmenter::new_dictionary(WordBreakInvariantOptions::default());
    words
        .iter()
        .enumerate()
        .flat_map(|(index, word)| {
            let core = word.text.trim_end_matches(|c: char| {
                c != '\u{0085}' && (c.is_whitespace() || c == '\u{feff}')
            });
            if core.encode_utf16().count() <= 5 || breaks.find(core).is_some() {
                return vec![word.clone()];
            }
            let mut boundaries = segmenter.segment_str(&word.text).collect::<Vec<_>>();
            if boundaries.len() <= 2 && grapheme.find(&word.text).is_some() {
                boundaries = GraphemeClusterSegmenter::new()
                    .segment_str(&word.text)
                    .collect();
            }
            if boundaries.len() <= 2 {
                return vec![word.clone()];
            }
            let end = word
                .end
                .or_else(|| words.get(index + 1).map(|word| word.time))
                .or(line_end)
                .unwrap_or(word.time);
            let duration = (end - word.time) / (boundaries.len() - 1) as f64;
            boundaries
                .windows(2)
                .enumerate()
                .map(|(index, range)| Word {
                    time: word.time + index as f64 * duration,
                    end: Some(word.time + (index + 1) as f64 * duration),
                    text: word.text[range[0]..range[1]].to_owned(),
                })
                .collect()
        })
        .collect()
}

pub fn word_is_rtl(text: &str) -> bool {
    use std::sync::OnceLock;
    static RTL: OnceLock<regress::Regex> = OnceLock::new();
    RTL.get_or_init(|| {
        regress::Regex::with_flags(
            r"[\p{Script=Arabic}\p{Script=Hebrew}\p{Script=Syriac}\p{Script=Thaana}]",
            "u",
        )
        .unwrap()
    })
    .find(text)
    .is_some()
}

pub fn word_progress(words: &[Word], index: usize, position: f64, line_end: Option<f64>) -> f32 {
    let Some(word) = words.get(index) else {
        return 0.;
    };
    if !position.is_finite() {
        return 0.;
    }
    let end = word
        .end
        .or_else(|| words.get(index + 1).map(|word| word.time))
        .or(line_end)
        .filter(|end| end.is_finite() && *end >= word.time)
        .unwrap_or(word.time);
    if end <= word.time {
        return if position >= word.time { 1. } else { 0. };
    }
    ((position - word.time) / (end - word.time)).clamp(0., 1.) as f32
}

/// Index-aligned translations take precedence; otherwise use the latest earlier timed line.
pub fn overlay_line(lines: &[Line], start: Option<f64>, index: usize) -> Option<&Line> {
    let start = start?;
    lines
        .get(index)
        .filter(|line| line.time.is_some_and(|time| time <= start))
        .or_else(|| active_line(lines, start).and_then(|index| lines.get(index)))
}

pub fn active_line(lines: &[Line], position: f64) -> Option<usize> {
    let mut active = None;
    for (index, line) in lines.iter().enumerate() {
        if let Some(time) = line.time {
            if position < time {
                break;
            }
            active = Some(index);
        }
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn word_cue_splitting_and_rtl_match_original_unicode_cases() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../assets/lyrics-word-segmentation-reference.json"
        ))
        .unwrap();
        for case in cases {
            let source = &case["word"];
            let word = Word {
                time: source["startMs"].as_f64().unwrap() / 1000.,
                end: Some(source["endMs"].as_f64().unwrap() / 1000.),
                text: source["text"].as_str().unwrap().into(),
            };
            assert_eq!(word_is_rtl(&word.text), case["rtl"] == true, "{case}");
            let actual = split_word_cues(&[word], None);
            let expected = case["expected"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{case}: {actual:?}");
            for (word, expected) in actual.iter().zip(expected) {
                assert_eq!(word.text, expected["text"].as_str().unwrap(), "{case}");
                assert!(
                    (word.time * 1000. - expected["startMs"].as_f64().unwrap()).abs() < 0.00001,
                    "{case}"
                );
                assert!(
                    (word.end.unwrap() * 1000. - expected["endMs"].as_f64().unwrap()).abs()
                        < 0.00001,
                    "{case}"
                );
            }
        }
    }

    #[test]
    fn richsync_singers_durations_overlays_and_legacy_cache() {
        use serde_json::json;
        let variant = structured_variant(&json!({"synced":true, "lang":"jpn", "line":[
            {"start":1000,"value":"朝 の歌"}, {"start":5000,"value":"次"}
        ], "agents":[{"id":"lead","name":"Singer","role":"lead"},{"id":"choir","name":"Choir","role":"bg"}],
        "cueLine":[
            {"index":0,"agentId":"lead","start":1000,"end":4000,"value":"朝 の歌","cue":[
                {"start":1000,"end":2000,"byteStart":0,"byteEnd":2},
                {"start":2000,"end":4000,"byteStart":4,"byteEnd":9}
            ]},
            {"index":0,"agentId":"choir","start":1500,"end":3000,"value":"Ah","cue":[{"start":1500,"end":3000,"value":"Ah"}]},
            {"index":1,"start":5000,"end":4000,"value":"次","cue":[{"start":5000,"end":4000,"byteStart":1,"byteEnd":1}]}
        ]}));
        assert_eq!(variant.agents[1].role, "bg");
        assert_eq!(variant.lines[0].cue_lines.len(), 2);
        assert!(variant.lines[0].words.is_empty());
        let lead = &variant.lines[0].cue_lines[0];
        assert_eq!(lead.agent_id.as_deref(), Some("lead"));
        assert_eq!(lead.words[0].text, "朝 ");
        assert_eq!(lead.words[1].end, Some(4.));
        for (time, expected) in [(0., 0.), (1., 0.), (1.5, 0.5), (2., 1.), (10., 1.)] {
            assert_eq!(word_progress(&lead.words, 0, time, lead.end), expected);
        }
        assert_eq!(word_progress(&lead.words, 1, 3., lead.end), 0.5);
        assert_eq!(word_progress(&lead.words, 9, 3., lead.end), 0.);
        assert_eq!(word_progress(&lead.words, 0, f64::NAN, lead.end), 0.);
        assert_eq!(variant.lines[1].cue_lines[0].end, None);
        assert!(variant.lines[1].cue_lines[0].words.is_empty());
        let old: Line = serde_json::from_value(
            json!({"time":1.,"text":"Old","words":[{"time":1.,"text":"Old"}]}),
        )
        .unwrap();
        assert!(old.cue_lines.is_empty());
        assert_eq!(old.words[0].end, None);
        assert_eq!(word_progress(&old.words, 0, 2., Some(3.)), 0.5);
        assert_eq!(word_progress(&old.words, 0, 2., None), 1.);
        let translated = parse_lrc("[00:01]One\n[00:04]Two");
        assert_eq!(overlay_line(&translated, Some(5.), 1).unwrap().text, "Two");
        assert_eq!(overlay_line(&translated, Some(2.), 1).unwrap().text, "One");
        assert!(overlay_line(&translated, Some(0.), 0).is_none());
        assert!(overlay_line(&translated, None, 0).is_none());
        let cached: Variant =
            serde_json::from_str(&serde_json::to_string(&variant).unwrap()).unwrap();
        assert_eq!(cached.lines, variant.lines);
        assert_eq!(cached.agents, variant.agents);
    }
    #[test]
    fn structured_languages_offsets_utf8_word_cues_and_follow_timing() {
        let variant = structured_variant(
            &serde_json::json!({"lang":"jpn","kind":"main","synced":true,"offset":250,"line":[{"start":1000,"value":"朝 の歌"},{"start":5000,"value":"次"}],"cueLine":[{"index":0,"value":"朝 の歌","cue":[{"start":1000,"end":2000,"byteStart":0,"byteEnd":2},{"start":2000,"end":4000,"byteStart":4,"byteEnd":9}]}]}),
        );
        assert_eq!(variant.language, "jpn");
        assert_eq!(variant.offset, Some(0.25));
        assert_eq!(variant.lines[0].words[0].text, "朝 ");
        assert_eq!(variant.lines[0].words[1].text, "の歌");
        assert_eq!(variant.lines[0].words[1].time, 2.0);
        assert_eq!(active_line(&variant.lines, 4.3), Some(0));
        assert_eq!(follow_line(&variant.lines, 4.3, 0.8), Some(1));
        assert_eq!(follow_line(&variant.lines, 0.0, 0.8), Some(0));
        assert_eq!(follow_ratio(-50.0), 0.05);
        assert_eq!(follow_ratio(50.0), 0.95);
        let plain = structured_variant(
            &serde_json::json!({"lang":"eng","synced":false,"line":[{"start":1000,"value":"Plain"}]}),
        );
        assert_eq!(plain.lines[0].time, None);
        assert_eq!(follow_line(&plain.lines, 10.0, 0.8), None);
    }
    #[test]
    fn raw_display_grammar_priority_order_and_follow_match_the_original() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lrc-offset-reference.json")).unwrap();
        for case in reference["formatCases"].as_array().unwrap() {
            let lines = display_lines(case["input"].as_str().unwrap());
            if let Some(expected) = case["expected"].as_array() {
                assert_eq!(lines.len(), expected.len(), "{case}");
                for (line, expected) in lines.iter().zip(expected) {
                    assert_eq!(
                        line.time,
                        Some(expected["startMs"].as_f64().unwrap() / 1000.),
                        "{case}"
                    );
                    assert_eq!(line.text, expected["text"].as_str().unwrap(), "{case}");
                }
                for trace in case["active"].as_array().unwrap() {
                    let index = active_line(&lines, trace["position"].as_f64().unwrap() / 1000.);
                    assert_eq!(
                        index.map_or(-1, |index| index as i64),
                        trace["index"].as_i64().unwrap(),
                        "{case}"
                    );
                }
            } else {
                assert_eq!(
                    lines
                        .iter()
                        .map(|line| line.text.as_str())
                        .collect::<Vec<_>>(),
                    case["expected"]
                        .as_str()
                        .unwrap()
                        .split('\n')
                        .collect::<Vec<_>>(),
                    "{case}"
                );
                assert!(lines.iter().all(|line| line.time.is_none()));
            }
        }
    }
    #[test]
    fn alternate_netease_timing_matches_the_original_formatter() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lrc-offset-reference.json")).unwrap();
        for case in reference["alternateCases"].as_array().unwrap() {
            let record = crate::lyric_sources::Record {
                raw: case["input"].as_str().unwrap().into(),
                matched: None,
                variants: vec![],
                selected: 0,
            };
            let lines = record.lines();
            let expected = case["expected"].as_array().unwrap();
            assert_eq!(lines.len(), expected.len(), "{case}");
            for (line, expected) in lines.iter().zip(expected) {
                assert_eq!(
                    line.time,
                    Some(expected["startMs"].as_f64().unwrap() / 1000.),
                    "{case}"
                );
                assert_eq!(line.text, expected["text"].as_str().unwrap(), "{case}");
            }
        }
    }
    #[test]
    fn unsynchronized_records_preserve_the_original_blank_lines_and_text() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lrc-offset-reference.json")).unwrap();
        for case in reference["plainCases"].as_array().unwrap() {
            let record = crate::lyric_sources::Record {
                raw: case["input"].as_str().unwrap().into(),
                matched: None,
                variants: vec![],
                selected: 0,
            };
            let lines = record.lines();
            assert_eq!(
                lines
                    .iter()
                    .map(|line| line.text.as_str())
                    .collect::<Vec<_>>(),
                case["expected"]
                    .as_str()
                    .unwrap()
                    .split('\n')
                    .collect::<Vec<_>>()
            );
            assert!(lines.iter().all(|line| line.time.is_none()));
        }
    }
    #[test]
    fn raw_lrc_offsets_match_the_original_display_formatter() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lrc-offset-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let lines = parse_lrc(case["input"].as_str().unwrap());
            let expected = case["expected"].as_array().unwrap();
            assert_eq!(lines.len(), expected.len());
            for (line, expected) in lines.iter().zip(expected) {
                assert_eq!(
                    line.time,
                    Some(expected["startMs"].as_f64().unwrap() / 1000.),
                    "{case}"
                );
                assert_eq!(line.text, expected["text"].as_str().unwrap());
            }
            let record = crate::lyric_sources::Record {
                raw: case["input"].as_str().unwrap().into(),
                matched: None,
                variants: vec![],
                selected: 0,
            };
            assert_eq!(record.lines(), lines);
        }
    }
    #[test]
    fn timestamps_duplicates_offsets_and_plain_text() {
        let lines = parse_lrc(
            "[ar:Artist]\n[offset:-500]\n[00:10.25][00:20.250]Repeated\n[00:02]First\nPlain\n[00:99]Invalid",
        );
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[1].time, Some(2.0));
        assert_eq!(lines[2].time, Some(10.25));
        assert_eq!(active_line(&lines, 10.0), Some(1));
        assert_eq!(parse_lrc("hello\nworld")[1].text, "world");
        let enhanced = parse_lrc("[offset:200]\n[01:02.00]<01:02.00>Hello <01:03.25>world");
        assert_eq!(enhanced[0].text, "Hello world");
        assert_eq!(enhanced[0].words[1].time, 63.25);
        assert_eq!(parse_lrc(&render_lrc(&enhanced)), enhanced);
        assert_eq!(parse_lrc("[offset:NaN]\n[00:01]Hi")[0].time, Some(1.0));
        assert_eq!(
            parse_lrc(&merge_translation("[00:01]Hello", "[00:01.00]Bonjour"))[0].text,
            "Hello · Bonjour"
        );
    }
}
