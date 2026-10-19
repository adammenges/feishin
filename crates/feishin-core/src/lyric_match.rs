// Fuse.js 7.4.2 Bitap scoring, adapted for the original lyric search options.
// Copyright 2017 Kirollos Risk. Apache-2.0; see assets/fuse-LICENSE.txt.
use crate::lyric_sources::Match;
use std::collections::HashMap;

fn bitap(
    text: &[u16],
    pattern: &[u16],
    location: usize,
    mut threshold: f64,
    ignore_location: bool,
) -> (bool, f64) {
    let expected = location.min(text.len());
    let score = |errors: usize, current: usize| {
        errors as f64 / pattern.len() as f64
            + if ignore_location {
                0.
            } else {
                expected.abs_diff(current) as f64 / 100.
            }
    };
    let mut exact = expected;
    while exact + pattern.len() <= text.len() {
        let Some(found) = text[exact..]
            .windows(pattern.len())
            .position(|part| part == pattern)
        else {
            break;
        };
        exact += found;
        threshold = threshold.min(score(0, exact));
        exact += pattern.len();
    }
    let mut alphabet = HashMap::<u16, u32>::new();
    for (index, unit) in pattern.iter().enumerate() {
        *alphabet.entry(*unit).or_default() |= 1 << (pattern.len() - index - 1);
    }
    let mask = 1 << (pattern.len() - 1);
    let mut previous = vec![];
    let mut final_score: f64 = 1.;
    let mut best = None;
    let mut bin_max = pattern.len() + text.len();
    for errors in 0..pattern.len() {
        let mut bin_min = 0;
        let mut bin_mid = bin_max;
        while bin_min < bin_mid {
            if score(errors, expected + bin_mid) <= threshold {
                bin_min = bin_mid;
            } else {
                bin_max = bin_mid;
            }
            bin_mid = (bin_max - bin_min) / 2 + bin_min;
        }
        bin_max = bin_mid;
        let mut start = (expected as isize - bin_mid as isize + 1).max(1) as usize;
        let finish = (expected + bin_mid).min(text.len()) + pattern.len();
        let mut bits = vec![0_u32; finish + 2];
        bits[finish + 1] = (1_u32 << errors) - 1;
        let mut index = finish;
        while index >= start {
            let current = index - 1;
            let char_match = text
                .get(current)
                .and_then(|unit| alphabet.get(unit))
                .copied()
                .unwrap_or_default();
            bits[index] = ((bits[index + 1] << 1) | 1) & char_match;
            if errors > 0 {
                let next = previous.get(index + 1).copied().unwrap_or(0);
                let here = previous.get(index).copied().unwrap_or(0);
                bits[index] |= ((next | here) << 1) | 1 | next;
            }
            if bits[index] & mask != 0 {
                final_score = score(errors, current);
                if final_score <= threshold {
                    threshold = final_score;
                    best = Some(current);
                    if current <= expected {
                        break;
                    }
                    start = (2 * expected as isize - current as isize).max(1) as usize;
                }
            }
            index -= 1;
        }
        if score(errors + 1, expected) > threshold {
            break;
        }
        previous = bits;
    }
    (best.is_some(), final_score.max(0.001))
}

fn field_score(query: &str, value: &str, weight: f64) -> Option<f64> {
    search_score(query, value, weight, 0.6, false)
}

pub(crate) fn search_score(
    query: &str,
    value: &str,
    weight: f64,
    threshold: f64,
    ignore_location: bool,
) -> Option<f64> {
    if query.is_empty() || value.trim().is_empty() {
        return None;
    }
    let pattern = query.to_lowercase().encode_utf16().collect::<Vec<_>>();
    let text = value.to_lowercase().encode_utf16().collect::<Vec<_>>();
    // Bound work for malformed provider metadata; ordinary titles are much shorter.
    if pattern.len() > 4096 || text.len() > 16384 {
        return None;
    }
    let base = if pattern == text {
        f64::EPSILON
    } else {
        let mut chunks = vec![];
        if pattern.len() <= 32 {
            chunks.push((&pattern[..], 0));
        } else {
            let end = pattern.len() / 32 * 32;
            for start in (0..end).step_by(32) {
                chunks.push((&pattern[start..start + 32], start));
            }
            if pattern.len() % 32 != 0 {
                let start = pattern.len() - 32;
                chunks.push((&pattern[start..], start));
            }
        }
        let mut found = false;
        let mut total = 0.;
        for (chunk, location) in &chunks {
            let (matched, score) = bitap(&text, chunk, *location, threshold, ignore_location);
            found |= matched;
            total += score;
        }
        if !found {
            return None;
        }
        total / chunks.len() as f64
    };
    let mut tokens = 1_usize;
    let mut in_space = false;
    for unit in value.encode_utf16() {
        if unit == 32 {
            if !in_space {
                tokens += 1;
                in_space = true;
            }
        } else {
            in_space = false;
        }
    }
    let norm = (1000. / (tokens as f64).sqrt()).round() / 1000.;
    Some(base.powf(weight * norm))
}

pub(crate) fn order(results: Vec<Match>, name: &str, artist: &str) -> Vec<Match> {
    let mut ranked = if !name.is_empty() && !artist.is_empty() {
        let sorted = |artist_field: bool| {
            let mut rows = results
                .iter()
                .enumerate()
                .filter_map(|(index, row)| {
                    field_score(
                        if artist_field { artist } else { name },
                        if artist_field { &row.artist } else { &row.name },
                        1.,
                    )
                    .map(|score| (index, score))
                })
                .collect::<Vec<_>>();
            rows.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            rows
        };
        let names = sorted(false)
            .into_iter()
            .map(|(index, score)| (results[index].id.clone(), score))
            .collect::<HashMap<_, _>>();
        let mut indices = HashMap::<String, usize>::new();
        let mut rows = vec![];
        for (index, score) in sorted(true) {
            let Some(name_score) = names.get(&results[index].id) else {
                continue;
            };
            let mut row = results[index].clone();
            row.score = score.max(*name_score);
            if let Some(index) = indices.get(&row.id) {
                rows[*index] = row;
            } else {
                indices.insert(row.id.clone(), rows.len());
                rows.push(row);
            }
        }
        rows
    } else {
        results
            .into_iter()
            .filter_map(|mut row| {
                row.score = if !name.is_empty() {
                    field_score(name, &row.name, 0.5)?
                } else {
                    field_score(artist, &row.artist, 0.5)?
                };
                Some(row)
            })
            .collect::<Vec<_>>()
    };
    ranked.sort_by(|a, b| {
        b.synchronized
            .cmp(&a.synchronized)
            .then(a.score.total_cmp(&b.score))
    });
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoring_and_order_match_the_original_typescript_fuse_reference() {
        #[derive(serde::Deserialize)]
        struct Case {
            name: String,
            artist: String,
            input: Vec<Match>,
            expected: Vec<Expected>,
        }
        #[derive(serde::Deserialize)]
        struct Expected {
            provider: crate::lyric_sources::Provider,
            id: String,
            score: f64,
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../assets/lyric-match-reference.json")).unwrap();
        assert!(cases.len() >= 400);
        for (index, case) in cases.into_iter().enumerate() {
            let rows = order(case.input, &case.name, &case.artist);
            assert_eq!(
                rows.len(),
                case.expected.len(),
                "case {index}: {} / {}",
                case.name,
                case.artist
            );
            for (row, expected) in rows.iter().zip(case.expected) {
                assert_eq!(
                    (row.provider, row.id.as_str()),
                    (expected.provider, expected.id.as_str()),
                    "case {index}"
                );
                assert!(
                    (row.score - expected.score).abs() < 1e-12,
                    "case {index}: {} != {}",
                    row.score,
                    expected.score
                );
            }
        }
    }
}
