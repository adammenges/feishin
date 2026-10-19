//! Kuroshiro 1.2.0's Hepburn table and ordered conversion rules (MIT).
use std::{collections::HashMap, sync::OnceLock};
pub(crate) fn hiragana(text: &str) -> String {
    text.chars()
        .map(|c| {
            if ('\u{30a1}'..='\u{30f6}').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap()
            } else {
                c
            }
        })
        .collect()
}
pub(crate) fn romanize(text: &str) -> String {
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/japanese-hepburn.json")).unwrap()
    });
    let mut kana = vec![];
    let chars = text.chars().collect::<Vec<_>>();
    for (index, &c) in chars.iter().enumerate() {
        kana.push(c);
        if matches!(c, 'ん' | 'ン')
            && chars.get(index + 1).is_some_and(|next| {
                "あいうえおアイウエオぁぃぅぇぉァィゥェォやゆよヤユヨゃゅょャュョ".contains(*next)
            })
        {
            kana.push('\'')
        }
    }
    let mut result = String::new();
    let mut index = 0;
    while index < kana.len() {
        let pair = kana[index..kana.len().min(index + 2)]
            .iter()
            .collect::<String>();
        if pair.chars().count() == 2
            && let Some(value) = table.get(&pair)
        {
            result.push_str(value);
            index += 2;
            continue;
        }
        let single = kana[index].to_string();
        result.push_str(table.get(&single).map_or(single.as_str(), String::as_str));
        index += 1;
    }
    let mut output = String::new();
    let mut chars = result.chars().peekable();
    while let Some(c) = chars.next() {
        if matches!(c, 'っ' | 'ッ') {
            if let Some(&next) = chars
                .peek()
                .filter(|next| "bcdfghijklmnopqrstuvwyz".contains(**next))
            {
                output.push(next)
            } else {
                output.push_str("tsu")
            }
        } else {
            output.push(c)
        }
    }
    output = output
        .replace("cc", "tc")
        .replace("nm", "mm")
        .replace("nb", "mb")
        .replace("np", "mp");
    for (from, to) in [
        ("aー", "ā"),
        ("iー", "ī"),
        ("uー", "ū"),
        ("eー", "ē"),
        ("oー", "ō"),
    ] {
        output = output.replace(from, to)
    }
    output
}
