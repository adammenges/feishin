//! The original table and detail views intentionally format some fields differently.
use crate::{lists, model::Item};
use chrono::{DateTime, Datelike, Local, Months, NaiveDate, NaiveDateTime, Offset, TimeZone, Utc};
use icu_datetime::{DateTimeFormatter, fieldsets, input::Date};
use icu_decimal::DecimalFormatter;
use icu_locale_core::Locale;
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, sync::OnceLock};

fn locales() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/metadata-date-locales.json")).unwrap()
    })
}

fn locale_data(language: &str) -> (&str, &'static Value) {
    let data = locales();
    let locale = data["mapping"][language].as_str().unwrap_or("en");
    (locale, &data["catalog"][locale])
}

use crate::i18n::system_locale;

fn intl_locale(language: &str, fallback: &str) -> Locale {
    // Chromium ships fewer Intl locales than ICU4X. Unsupported requested
    // languages use the system locale, independently of the Day.js locale.
    let supported = |language: &str| {
        let data = &locales()["intl"];
        data.get(language).or_else(|| {
            language
                .split_once('-')
                .and_then(|(base, _)| data.get(base))
        }) != Some(&Value::Bool(false))
    };
    let language = if supported(language) {
        language
    } else if supported(fallback) {
        fallback
    } else {
        "en-US"
    };
    (if language == "tl" { "fil" } else { language })
        .parse()
        .unwrap_or_else(|_| "en".parse().unwrap())
}

pub fn number(value: &str, language: &str) -> String {
    thread_local! {
        static FORMATTERS: RefCell<HashMap<String, DecimalFormatter>> = RefCell::new(HashMap::new());
    }
    let Ok(decimal) = value.parse::<fixed_decimal::Decimal>() else {
        return value.into();
    };
    FORMATTERS.with_borrow_mut(|formatters| {
        let formatter = formatters.entry(language.into()).or_insert_with(|| {
            DecimalFormatter::try_new(intl_locale(language, "en-US").into(), Default::default())
                .unwrap()
        });
        formatter.format(&decimal).to_string()
    })
}

pub fn size(mut value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let mut unit = 0;
    while value > 1024. {
        value /= 1024.;
        unit += 1;
    }
    // JS toFixed rounds the exact binary value, with ties away from zero.
    let bits = value.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 - 52;
    let mantissa = ((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52)) as u128 * 100;
    let cents = if exponent <= -128 {
        0
    } else if exponent < 0 {
        let divisor = 1_u128 << -exponent;
        (mantissa + divisor / 2) / divisor
    } else {
        mantissa.checked_shl(exponent as u32).unwrap_or(0)
    };
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    format!(
        "{}{}.{:02} {}",
        if value < 0. { "-" } else { "" },
        cents / 100,
        cents % 100,
        units.get(unit).unwrap_or(&"undefined")
    )
}

fn local_datetime<Tz: TimeZone>(value: NaiveDateTime, timezone: &Tz) -> Option<DateTime<Tz>> {
    // JavaScript chooses the earlier offset for repeated times and the offset
    // before a gap for skipped times, including half-hour and whole-day changes.
    timezone.from_local_datetime(&value).earliest().or_else(|| {
        let previous = timezone
            .from_local_datetime(&(value - chrono::Duration::days(1)))
            .earliest()?;
        Some(
            previous
                .offset()
                .fix()
                .from_local_datetime(&value)
                .single()?
                .with_timezone(timezone),
        )
    })
}

fn parse<Tz: TimeZone>(raw: &str, timezone: &Tz) -> Option<DateTime<Tz>> {
    let raw = raw.trim();
    if raw.len() > 1024 {
        return None;
    }
    if let Ok(value) =
        DateTime::parse_from_rfc3339(raw).or_else(|_| DateTime::parse_from_rfc2822(raw))
    {
        return Some(value.with_timezone(timezone));
    }
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| regex::Regex::new(r"^(\d{4})[-/]?(\d{1,2})?[-/]?(\d{0,2})[Tt\s]*(\d{1,2})?:?(\d{1,2})?:?(\d{1,2})?[.:]?(\d+)?$").unwrap());
    let captures = pattern.captures(raw)?;
    let part = |index, default| {
        captures
            .get(index)
            .and_then(|value| value.as_str().parse::<i64>().ok())
            .unwrap_or(default)
    };
    let year = part(1, 0);
    let year = if year < 100 { year + 1900 } else { year };
    let month = part(2, 1) - 1;
    let day = NaiveDate::from_ymd_opt(
        (year + month.div_euclid(12)) as i32,
        (month.rem_euclid(12) + 1) as u32,
        1,
    )?;
    let milliseconds = captures
        .get(7)
        .and_then(|value| value.as_str().get(..value.len().min(3)))
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    let value = day.and_hms_opt(0, 0, 0)?
        + chrono::Duration::days(part(3, 1) - 1)
        + chrono::Duration::hours(part(4, 0))
        + chrono::Duration::minutes(part(5, 0))
        + chrono::Duration::seconds(part(6, 0))
        + chrono::Duration::milliseconds(milliseconds);
    local_datetime(value, timezone)
}

fn absolute<Tz: TimeZone>(raw: &str, language: &str, timezone: &Tz) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let Some(date) = parse(raw, timezone) else {
        return "Invalid Date".into();
    };
    locale_data(language).1["date"][date.month0() as usize]
        .as_str()
        .unwrap()
        .replace("{year}", &format!("{:04}", date.year()))
        .replace("{day}", &date.day().to_string())
}

pub fn date(raw: &str, language: &str) -> String {
    date_with_locale(raw, language, system_locale())
}

fn date_with_locale(raw: &str, language: &str, fallback: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let raw = raw.trim();
    let year = raw.len() == 4 && raw.bytes().all(|byte| byte.is_ascii_digit());
    let month = raw.len() == 7
        && raw.as_bytes()[4] == b'-'
        && raw
            .bytes()
            .enumerate()
            .all(|(i, byte)| i == 4 || byte.is_ascii_digit());
    if !year && !month {
        return absolute(raw, language, &Utc);
    }
    let Some(date) = parse(raw, &Utc) else {
        return raw.into();
    };
    let Ok(date) = Date::try_new_iso(date.year(), date.month() as u8, date.day() as u8) else {
        return raw.into();
    };
    thread_local! {
        static YEARS: RefCell<HashMap<String, DateTimeFormatter<fieldsets::Y>>> = RefCell::new(HashMap::new());
        static MONTHS: RefCell<HashMap<String, DateTimeFormatter<fieldsets::YM>>> = RefCell::new(HashMap::new());
    }
    let locale = intl_locale(language, fallback);
    let cache_key = locale.to_string();
    let text = if year {
        YEARS.with_borrow_mut(|cache| {
            cache
                .entry(cache_key)
                .or_insert_with(|| {
                    DateTimeFormatter::try_new(locale.into(), fieldsets::Y::long()).unwrap()
                })
                .format(&date)
                .to_string()
        })
    } else {
        MONTHS.with_borrow_mut(|cache| {
            cache
                .entry(cache_key)
                .or_insert_with(|| {
                    DateTimeFormatter::try_new(locale.into(), fieldsets::YM::long()).unwrap()
                })
                .format(&date)
                .to_string()
        })
    };
    // Chromium's Intl retains a plain space before these year suffixes.
    if matches!(language, "bg" | "ru" | "uk") {
        text.replace('\u{202f}', " ")
    } else {
        text
    }
}

fn add_months<Tz: TimeZone>(date: &DateTime<Tz>, months: i32) -> Option<DateTime<Tz>> {
    let value = if months < 0 {
        date.naive_local()
            .checked_sub_months(Months::new(months.unsigned_abs()))
    } else {
        date.naive_local()
            .checked_add_months(Months::new(months as u32))
    }?;
    local_datetime(value, &date.timezone())
}

fn month_difference<Tz: TimeZone>(a: &DateTime<Tz>, b: &DateTime<Tz>) -> f64 {
    if a.day() < b.day() {
        return -month_difference(b, a);
    }
    let months = (b.year() - a.year()) * 12 + b.month() as i32 - a.month() as i32;
    let Some(anchor) = add_months(a, months) else {
        return 0.;
    };
    let before = b < &anchor;
    let Some(other) = add_months(a, months + if before { -1 } else { 1 }) else {
        return 0.;
    };
    let remainder = b.clone().signed_duration_since(&anchor).num_milliseconds() as f64;
    let width = if before {
        anchor.signed_duration_since(other)
    } else {
        other.signed_duration_since(anchor)
    }
    .num_milliseconds() as f64;
    -(f64::from(months) + remainder / width)
}

fn relative<Tz: TimeZone>(raw: &str, language: &str, now: &DateTime<Tz>) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let Some(value) = parse(raw, &now.timezone()) else {
        return String::new();
    };
    let seconds = value.clone().signed_duration_since(now).num_milliseconds() as f64 / 1000.;
    let offset =
        f64::from(now.offset().fix().local_minus_utc() - value.offset().fix().local_minus_utc());
    let mut difference = seconds;
    let thresholds = [
        ("s", 44., 1.),
        ("m", 89., 0.),
        ("mm", 44., 60.),
        ("h", 89., 0.),
        ("hh", 21., 3600.),
        ("d", 35., 0.),
        ("dd", 25., 86400.),
        ("M", 45., 0.),
        ("MM", 10., -1.),
        ("y", 17., 0.),
        ("yy", f64::INFINITY, -12.),
    ];
    for (index, (key, limit, unit)) in thresholds.iter().enumerate() {
        difference = match *unit {
            0. => difference,
            -1. => month_difference(&value, now),
            -12. => month_difference(&value, now) / 12.,
            86400. => (seconds - offset) / unit,
            _ => seconds / unit,
        };
        let count = difference.abs().round() as u64;
        if count as f64 > *limit {
            continue;
        }
        let key = if count <= 1 && index > 0 {
            thresholds[index - 1].0
        } else {
            key
        };
        let (locale, data) = locale_data(language);
        let term = &data["relative"][if difference > 0. { "future" } else { "past" }][key];
        return if count < 10 {
            term["small"][count as usize].as_str().unwrap().into()
        } else {
            let plural = crate::i18n::plural(locale, &count.to_string()).unwrap_or("other");
            term["forms"][plural]
                .as_str()
                .or_else(|| term["forms"]["other"].as_str())
                .unwrap()
                .replace("{n}", &count.to_string())
        };
    }
    String::new()
}

pub fn column_text(item: &Item, id: &str, index: usize, detail: bool, language: &str) -> String {
    column_text_at(
        item,
        id,
        index,
        detail,
        language,
        system_locale(),
        &Local::now(),
    )
}

fn column_text_at<Tz: TimeZone>(
    item: &Item,
    id: &str,
    index: usize,
    detail: bool,
    language: &str,
    number_locale: &str,
    now: &DateTime<Tz>,
) -> String {
    let raw = item.column_data.get(id).map(String::as_str).unwrap_or("");
    let numeric = |fallback: u32| {
        item.column_data
            .get(id)
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    };
    match id {
        "duration" => crate::model::duration_label(item.duration),
        "size" => raw
            .parse::<f64>()
            .ok()
            .filter(|value| !detail || *value != 0.)
            .map(size)
            .unwrap_or_default(),
        "bitRate" if detail && !raw.is_empty() => format!("{raw} kbps"),
        "sampleRate" if detail => {
            if !raw.is_empty() && raw != "0" {
                format!("{raw} Hz")
            } else {
                String::new()
            }
        }
        "bitRate" | "sampleRate" | "bitDepth" => raw.into(),
        "discNumber" => {
            let value = numeric(item.disc);
            if detail && value.is_empty() {
                "1".into()
            } else {
                value
            }
        }
        "playCount" => {
            let value = numeric(item.play_count);
            if detail {
                if value == "0" { String::new() } else { value }
            } else {
                number(&value, number_locale)
            }
        }
        "albumCount" | "songCount" => number(raw, number_locale),
        "year" => {
            let value = item.column_data.get(id).cloned().unwrap_or_else(|| {
                if item.year > 0 {
                    item.year.to_string()
                } else {
                    String::new()
                }
            });
            if detail && value == "0" {
                String::new()
            } else {
                value
            }
        }
        "releaseYear" => {
            if detail {
                return if raw == "0" {
                    String::new()
                } else {
                    raw.into()
                };
            }
            let original = item.column_data.get("originalYear").filter(|original| {
                original.parse::<u32>().is_ok_and(|year| year > 0) && original.as_str() != raw
            });
            if !raw.is_empty()
                && let Some(original) = original
            {
                format!("{original} • {raw}")
            } else {
                raw.into()
            }
        }
        "date" | "originalDate" => date_with_locale(raw, language, number_locale),
        "releaseDate" => {
            let release = date_with_locale(raw, language, number_locale);
            let original = item
                .column_data
                .get("originalDate")
                .filter(|original| !original.is_empty() && original.as_str() != raw);
            if !raw.is_empty()
                && let Some(original) = original
            {
                format!(
                    "{} • {release}",
                    date_with_locale(original, language, number_locale)
                )
            } else {
                release
            }
        }
        "createdAt" | "updatedAt" => absolute(raw, language, &now.timezone()),
        "lastPlayedAt" => relative(raw, language, now),
        _ => lists::text(item, id, index),
    }
}

pub fn duration_text(seconds: f64, language: &str, short: bool) -> String {
    let clock = crate::model::duration_label(seconds);
    let parts = clock.split(':').collect::<Vec<_>>();
    let units = [
        "datetime.dayShort",
        "datetime.hourShort",
        "datetime.minuteShort",
        "datetime.secondShort",
    ];
    parts
        .iter()
        .zip(&units[4 - parts.len()..])
        .enumerate()
        .filter_map(|(index, (part, unit))| {
            if short && index >= 2 {
                return None;
            }
            let part = if short {
                *part
            } else {
                part.strip_prefix('0').unwrap_or(part)
            };
            (!part.is_empty()).then(|| {
                format!(
                    "{part}{}",
                    crate::i18n::translate(language, unit).unwrap_or_else(|| (*unit).into())
                )
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn count_text(id: &str, count: u64, language: &str) -> String {
    let key = match id {
        "songCount" => "entity.trackWithCount",
        "albumCount" => "entity.albumWithCount",
        _ => "entity.play",
    };
    crate::i18n::translate_with(language, key, &serde_json::json!({"count":count}))
        .unwrap_or_else(|| key.into())
}

pub fn card_text(item: &Item, id: &str, index: usize, language: &str) -> String {
    card_text_at(item, id, index, language, &Local::now())
}
fn card_text_at<Tz: TimeZone>(
    item: &Item,
    id: &str,
    index: usize,
    language: &str,
    now: &DateTime<Tz>,
) -> String {
    match id {
        "duration" => duration_text(item.duration, language, false),
        "playCount" => count_text(id, item.play_count as u64, language),
        "songCount" | "albumCount" => item
            .column_data
            .get(id)
            .and_then(|value| value.parse().ok())
            .map(|count| count_text(id, count, language))
            .unwrap_or_default(),
        "year" => {
            if item.year > 0 {
                item.year.to_string()
            } else {
                String::new()
            }
        }
        "userFavorite" => {
            if item.favorite {
                "★".into()
            } else {
                String::new()
            }
        }
        _ => column_text_at(item, id, index, false, language, system_locale(), now),
    }
}

pub fn player_text(item: &Item, column: &str, language: &str) -> String {
    let raw = item
        .column_data
        .get(column)
        .map(String::as_str)
        .unwrap_or("");
    let numeric = |value: String, suffix: &str, prefix: bool| {
        if value.is_empty() || value == "0" {
            value
        } else if prefix {
            format!("{suffix} {value}")
        } else {
            format!("{value} {suffix}")
        }
    };
    match column {
        "bitRate" => numeric(raw.into(), "kbps", false),
        "bitDepth" => numeric(raw.into(), "bit", false),
        "sampleRate" => numeric(
            raw.parse::<f64>()
                .map(|number| (number / 1000.).to_string())
                .unwrap_or_default(),
            "kHz",
            false,
        ),
        "bpm" => numeric(
            raw.into(),
            &crate::i18n::translate(language, "common.bpm").unwrap_or_else(|| "BPM".into()),
            false,
        ),
        "discNumber" => numeric(
            item.disc.to_string(),
            &crate::i18n::translate(language, "common.disc").unwrap_or_else(|| "Disc".into()),
            true,
        ),
        "trackNumber" => numeric(
            item.track.to_string(),
            &crate::i18n::translate(language, "common.trackNumber")
                .unwrap_or_else(|| "Track".into()),
            true,
        ),
        "date" | "releaseDate" => date(raw, language),
        "releaseYear" => raw.into(),
        "releaseType" => item
            .tags
            .get("releasetype")
            .and_then(|values| values.first())
            .cloned()
            .unwrap_or_default(),
        _ => lists::text(item, column, 0),
    }
}

pub fn album_tag_labels(item: &Item, language: &str) -> Vec<String> {
    let mut types = item.release_types.clone();
    if item.is_compilation
        && !types
            .iter()
            .any(|value| value.eq_ignore_ascii_case("compilation"))
    {
        types.push("compilation".into());
    }
    let mut values = crate::releases::metadata_labels(&types, language)
        .iter()
        .map(|value| crate::i18n::title_case(value))
        .collect::<Vec<_>>();
    for name in ["releasecountry", "releasestatus"] {
        values.extend(item.tags.get(name).into_iter().flatten().cloned());
    }
    if let Some(status) = item.explicit_status() {
        values.push(
            crate::i18n::translate(
                language,
                if status == "EXPLICIT" {
                    "common.explicit"
                } else {
                    "common.clean"
                },
            )
            .unwrap_or_default(),
        );
    }
    values
}

pub fn artist_header(item: &Item, language: &str) -> Vec<(String, String)> {
    let mut rows = ["albumCount", "songCount"]
        .into_iter()
        .filter_map(|id| {
            let count = item.column_data.get(id)?.parse().ok()?;
            Some((id.into(), count_text(id, count, language)))
        })
        .collect::<Vec<_>>();
    if item
        .column_data
        .get("duration")
        .is_some_and(|value| !value.is_empty())
    {
        rows.push((
            "duration".into(),
            duration_text(item.duration, language, false),
        ));
    }
    rows
}

pub fn album_header(item: &Item, language: &str) -> Vec<(String, String)> {
    let raw = |id| item.column_data.get(id).map(String::as_str).unwrap_or("");
    let mut rows = Vec::new();
    if let Ok(range) = serde_json::from_str::<Value>(raw("trackYearRange"))
        && range["min"] != range["max"]
    {
        rows.push((
            "trackYearRange".into(),
            format!("{}-{}", range["min"], range["max"]),
        ));
    }
    if !raw("originalDate").is_empty() && raw("originalDate") != raw("releaseDate") {
        rows.push(("originalDate".into(), date(raw("originalDate"), language)));
    } else if raw("originalYear")
        .parse::<u32>()
        .is_ok_and(|year| year > 0)
        && !raw("releaseYear").is_empty()
        && raw("originalYear") != raw("releaseYear")
    {
        rows.push(("originalYear".into(), raw("originalYear").into()));
    }
    let prefix = if rows.is_empty() {
        String::new()
    } else {
        format!(
            "{} ",
            crate::i18n::translate(language, "page.albumDetail.released")
                .unwrap_or_else(|| "Released".into())
        )
    };
    if !raw("releaseDate").is_empty() {
        rows.push((
            "releaseDate".into(),
            format!("{prefix}{}", date(raw("releaseDate"), language)),
        ));
    } else if raw("releaseYear").parse::<u32>().is_ok_and(|year| year > 0) {
        rows.push((
            "releaseYear".into(),
            format!("{prefix}{}", raw("releaseYear")),
        ));
    }
    rows.push((
        "songCount".into(),
        count_text(
            "songCount",
            raw("songCount").parse().unwrap_or_default(),
            language,
        ),
    ));
    rows.push((
        "duration".into(),
        duration_text(item.duration, language, false),
    ));
    if let Ok(value) = raw("size").parse::<f64>()
        && value > 0.
    {
        rows.push(("size".into(), size(value)));
    }
    if item.play_count > 0 {
        rows.push((
            "playCount".into(),
            count_text("playCount", item.play_count as u64, language),
        ));
    }
    rows
}

pub fn information_text(item: &Item, id: &str, index: usize, language: &str) -> String {
    information_text_at(item, id, index, language, &Local::now())
}
fn information_text_at<Tz: TimeZone>(
    item: &Item,
    id: &str,
    index: usize,
    language: &str,
    now: &DateTime<Tz>,
) -> String {
    let raw = item.column_data.get(id).map(String::as_str).unwrap_or("");
    match id {
        "duration" => {
            if item.kind != crate::model::ItemKind::Song && item.duration == 0. {
                String::new()
            } else {
                duration_text(item.duration, language, false)
            }
        }
        "date" | "releaseDate" => date(raw, language),
        "year"
            if item
                .column_data
                .get("date")
                .is_some_and(|value| !value.is_empty()) =>
        {
            String::new()
        }
        "releaseYear"
            if item.kind == crate::model::ItemKind::Song
                && (raw == "0"
                    || item
                        .column_data
                        .get("releaseDate")
                        .is_some_and(|value| !value.is_empty())) =>
        {
            String::new()
        }
        "releaseYear" => raw.into(),
        "bitRate" => format!("{raw} kbps"),
        "sampleRate" | "bitDepth" => raw.into(),
        "size" => {
            if item.kind == crate::model::ItemKind::Song {
                size(raw.parse().unwrap_or_default())
            } else {
                raw.parse::<f64>()
                    .ok()
                    .filter(|value| *value > 0.)
                    .map(size)
                    .unwrap_or_default()
            }
        }
        "playCount" => item.play_count.to_string(),
        "lastPlayedAt" | "updatedAt" => relative(raw, language, now),
        _ => lists::text(item, id, index),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn surface_item(row: &Value) -> Item {
        Item {
            kind: crate::model::ItemKind::Song,
            name: row["name"].as_str().unwrap_or_default().into(),
            album: row["album"].as_str().unwrap_or_default().into(),
            year: row["year"].as_u64().unwrap_or_default() as u32,
            disc: row["discNumber"].as_u64().unwrap_or_default() as u32,
            track: row["trackNumber"].as_u64().unwrap_or_default() as u32,
            play_count: row["playCount"].as_u64().unwrap_or_default() as u32,
            duration: row["duration"].as_f64().unwrap_or_default() / 1000.,
            column_data: row
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        match value {
                            Value::Null => String::new(),
                            Value::String(value) => value.clone(),
                            _ => value.to_string(),
                        },
                    )
                })
                .collect(),
            tags: serde_json::from_value(row["tags"].clone()).unwrap(),
            release_types: serde_json::from_value(row["releaseTypes"].clone()).unwrap_or_default(),
            is_compilation: row["isCompilation"].as_bool().unwrap_or_default(),
            genre_links: Some(
                row["genres"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|genre| {
                        (
                            genre["id"].as_str().unwrap().into(),
                            genre["name"].as_str().unwrap().into(),
                        )
                    })
                    .collect(),
            ),
            ..Default::default()
        }
    }
    #[test]
    fn original_metadata_surfaces_keep_their_localized_field_formats() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/metadata-surfaces-reference.json"))
                .unwrap();
        let now = "2024-03-31T00:30:00Z".parse::<DateTime<Utc>>().unwrap();
        let mut failures = Vec::new();
        macro_rules! check {
            ($left:expr,$right:expr,$($args:tt)*) => {{ let left=$left; let right=$right; if left != right { failures.push(format!("{}: {:?} != {:?}",format!($($args)*),left,right)); } }};
        }
        for case in reference["durations"].as_array().unwrap() {
            for short in [false, true] {
                check!(
                    duration_text(
                        case["milliseconds"].as_f64().unwrap() / 1000.,
                        case["language"].as_str().unwrap(),
                        short
                    ),
                    case[if short { "short" } else { "long" }].as_str().unwrap(),
                    "duration {short} {case}"
                );
            }
        }
        for case in reference["cards"].as_array().unwrap() {
            let item = surface_item(&case["row"]);
            for (id, value) in case["fields"].as_object().unwrap() {
                check!(
                    card_text_at(&item, id, 0, case["language"].as_str().unwrap(), &now),
                    value.as_str().unwrap(),
                    "card {id} {case}"
                );
            }
        }
        let mut player = crate::player_display::Config::default();
        for field in &mut player.fields {
            field.disabled = false;
        }
        for case in reference["players"].as_array().unwrap() {
            let item = surface_item(&case["row"]);
            let metadata = player.metadata(&item, case["language"].as_str().unwrap());
            for (id, value) in case["fields"].as_object().unwrap() {
                let label = crate::player_display::FIELDS
                    .iter()
                    .find(|(field, _, _)| *field == id)
                    .unwrap()
                    .1;
                let actual = metadata
                    .iter()
                    .filter(|(name, _)| *name == label)
                    .map(|(_, value)| value.as_str())
                    .collect::<String>();
                check!(actual, value.as_str().unwrap(), "player {id} {case}");
            }
        }
        for case in reference["headers"].as_array().unwrap() {
            let item = surface_item(&case["row"]);
            check!(
                serde_json::json!(album_header(&item, case["language"].as_str().unwrap())),
                case["fields"].clone(),
                "header {case}"
            );
        }
        for case in reference["artistHeaders"].as_array().unwrap() {
            let item = surface_item(&case["row"]);
            check!(
                serde_json::json!(artist_header(&item, case["language"].as_str().unwrap())),
                case["fields"].clone(),
                "artist header {case}"
            );
        }
        for case in reference["albumTags"].as_array().unwrap() {
            let item = surface_item(&case["row"]);
            let expected = case["fields"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|field| field[0].as_str().unwrap().starts_with("tag-"))
                .map(|field| field[1].as_str().unwrap())
                .collect::<Vec<_>>();
            check!(
                album_tag_labels(&item, case["language"].as_str().unwrap()),
                expected,
                "album tags {case}"
            );
        }
        for case in reference["playerVisibility"].as_array().unwrap() {
            let item = surface_item(&case["row"]);
            for field in &mut player.fields {
                field.disabled = !case["enabled"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|id| id == &field.id);
            }
            check!(
                player
                    .metadata(&item, case["language"].as_str().unwrap())
                    .into_iter()
                    .map(|(_, text)| text)
                    .collect::<String>(),
                case["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|text| text.as_str().unwrap())
                    .collect::<String>(),
                "player visibility {case}"
            );
        }
        let mut group = crate::album_metadata::Config::default();
        for field in &mut group.items {
            field.disabled = false;
        }
        for case in reference["groups"].as_array().unwrap() {
            let songs = case["songs"]
                .as_array()
                .unwrap()
                .iter()
                .map(surface_item)
                .collect::<Vec<_>>();
            let rows = group.rows(&songs[0], &songs, case["language"].as_str().unwrap());
            for (id, value) in case["fields"].as_object().unwrap() {
                let actual = rows
                    .iter()
                    .find(|(key, _)| key == id)
                    .map_or("", |(_, value)| value.as_str());
                check!(actual, value.as_str().unwrap(), "group {id} {case}");
            }
        }
        for case in reference["info"].as_array().unwrap() {
            let mut item = surface_item(&case["row"]);
            item.kind = match case["kind"].as_str().unwrap() {
                "song" => crate::model::ItemKind::Song,
                "album" => crate::model::ItemKind::Album,
                kind => panic!("Unexpected item kind {kind}"),
            };
            for (id, value) in case["fields"].as_object().unwrap() {
                let field = match id.as_str() {
                    "common.duration" => "duration",
                    "filter.date" => "date",
                    "filter.year" => "year",
                    "filter.releaseDate" => "releaseDate",
                    "filter.releaseYear" => "releaseYear",
                    "common.bitrate" => "bitRate",
                    "common.sampleRate" => "sampleRate",
                    "common.bitDepth" => "bitDepth",
                    "common.size" => "size",
                    "filter.playCount" => "playCount",
                    "filter.lastPlayed" => "lastPlayedAt",
                    "common.modified" => "updatedAt",
                    _ => panic!("Unexpected field"),
                };
                check!(
                    information_text_at(&item, field, 0, case["language"].as_str().unwrap(), &now),
                    value.as_str().unwrap(),
                    "information {id} {case}"
                );
            }
        }
        assert!(
            failures.is_empty(),
            "{} differences:\n{}",
            failures.len(),
            failures
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    #[cfg(unix)]
    #[test]
    fn local_dates_follow_system_timezone() {
        let Some(zone) = std::env::var_os("FEISHIN_TEST_METADATA_ZONE") else {
            for zone in [
                "UTC",
                "America/Los_Angeles",
                "Europe/Berlin",
                "Asia/Kolkata",
            ] {
                let output = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "metadata_format::tests::local_dates_follow_system_timezone",
                        "--nocapture",
                    ])
                    .env("TZ", zone)
                    .env("FEISHIN_TEST_METADATA_ZONE", zone)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{zone}: {} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        };
        let reference: Value =
            serde_json::from_str(include_str!("../assets/metadata-format-reference.json")).unwrap();
        let now = reference["now"]
            .as_str()
            .unwrap()
            .parse::<DateTime<Utc>>()
            .unwrap()
            .with_timezone(&Local);
        for case in reference["zones"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["timezone"].as_str() == zone.to_str())
        {
            let raw = case["value"].as_str().unwrap();
            let language = case["language"].as_str().unwrap();
            assert_eq!(
                absolute(raw, language, &Local),
                case["absolute"].as_str().unwrap(),
                "{case}"
            );
            assert_eq!(
                relative(raw, language, &now),
                case["relative"].as_str().unwrap(),
                "{case}"
            );
        }
    }
    #[test]
    fn song_metadata_matches_original_backend_normalizers() {
        use crate::{
            api::Api,
            model::{ItemKind, Server, ServerType, Session},
        };
        let cases: Value =
            serde_json::from_str(include_str!("../assets/metadata-song-reference.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let kind: ServerType = serde_json::from_value(case["backend"].clone()).unwrap();
            let api = Api::new(
                Server {
                    id: uuid::Uuid::new_v4(),
                    name: "Metadata fixture".into(),
                    url: "http://127.0.0.1:1".into(),
                    username: "fixture".into(),
                    kind,
                    legacy_auth: false,
                    options: Default::default(),
                },
                Session::default(),
            )
            .unwrap();
            let item = api.normalize(&case["input"], ItemKind::Song);
            for (field, expected) in case["expected"].as_object().unwrap() {
                let expected = match expected {
                    Value::Null => String::new(),
                    Value::String(value) => value.clone(),
                    _ => expected.to_string(),
                };
                let actual = match field.as_str() {
                    "playCount" => item.play_count.to_string(),
                    "albumId" => item.album_id.clone(),
                    _ => item.column_data.get(field).cloned().unwrap_or_default(),
                };
                assert_eq!(actual, expected, "{field}: {case}");
            }
            assert_eq!(
                item.year,
                case["expected"]["year"].as_u64().unwrap_or_default() as u32
            );
            assert_eq!(
                item.disc,
                case["expected"]["discNumber"].as_u64().unwrap_or_default() as u32
            );
        }
    }
    #[test]
    fn original_metadata_formats_match_all_supported_locales() {
        let mut failures = Vec::new();
        macro_rules! check {
            ($left:expr, $right:expr, $($args:tt)*) => {{
                let left = $left; let right = $right;
                if left != right { failures.push(format!("{}: {:?} != {:?}", format!($($args)*), left, right)); }
            }};
        }
        let reference: Value =
            serde_json::from_str(include_str!("../assets/metadata-format-reference.json")).unwrap();
        let now = reference["now"]
            .as_str()
            .unwrap()
            .parse::<DateTime<Utc>>()
            .unwrap();
        for case in reference["counts"].as_array().unwrap() {
            check!(
                number(
                    &case["number"].to_string(),
                    case["language"].as_str().unwrap()
                ),
                case["text"].as_str().unwrap(),
                "{case}"
            );
        }
        for case in reference["dates"].as_array().unwrap() {
            let raw = case["value"].as_str().unwrap_or_default();
            let language = case["language"].as_str().unwrap();
            check!(
                absolute(raw, language, &Utc),
                case["absolute"].as_str().unwrap(),
                "{case}"
            );
            check!(
                date_with_locale(raw, language, "en-US"),
                case["partial"].as_str().unwrap(),
                "{case}"
            );
        }
        for case in reference["relative"].as_array().unwrap() {
            check!(
                relative(
                    case["value"].as_str().unwrap(),
                    case["language"].as_str().unwrap(),
                    &now
                ),
                case["text"].as_str().unwrap(),
                "{case}"
            );
        }
        for case in reference["cases"].as_array().unwrap() {
            let row = &case["row"];
            let item = Item {
                year: row["year"].as_u64().unwrap_or_default() as u32,
                disc: row["discNumber"].as_u64().unwrap_or_default() as u32,
                play_count: row["playCount"].as_u64().unwrap_or_default() as u32,
                duration: row["duration"].as_f64().unwrap_or_default() / 1000.,
                column_data: row
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            match value {
                                Value::String(value) => value.clone(),
                                Value::Null => String::new(),
                                _ => value.to_string(),
                            },
                        )
                    })
                    .collect(),
                ..Default::default()
            };
            for detail in [false, true] {
                let Some(expected) = case[if detail { "detail" } else { "table" }].as_str() else {
                    continue;
                };
                check!(
                    column_text_at(
                        &item,
                        case["id"].as_str().unwrap(),
                        0,
                        detail,
                        case["language"].as_str().unwrap_or("en"),
                        "en-US",
                        &now
                    ),
                    expected,
                    "detail={detail} {case}"
                );
            }
        }
        assert!(
            failures.is_empty(),
            "{} differences:\n{}",
            failures.len(),
            failures
                .iter()
                .take(50)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
