//! The original lyric export format, shared by the preview, clipboard and saved file.
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub artist: String,
    pub name: String,
    pub offset_ms: Option<f64>,
    pub lyrics: Value,
}

pub fn render(metadata: &Metadata, offset_ms: f64, synchronized: bool) -> Result<String> {
    if let Some(raw) = metadata.lyrics.as_str() {
        return Ok(raw.into());
    }
    let Some(lines) = metadata.lyrics.as_array() else {
        bail!("Lyrics must be text or an array of lines")
    };
    let mut output = Vec::with_capacity(lines.len());
    for line in lines {
        let (time, text) = if let Some(tuple) = line.as_array() {
            (tuple.first(), tuple.get(1))
        } else {
            (line.get("startMs"), line.get("text"))
        };
        let text = text.and_then(Value::as_str).unwrap_or_default();
        output.push(if synchronized {
            let time = time
                .and_then(Value::as_f64)
                .ok_or_else(|| anyhow::anyhow!("A lyric line has no timestamp"))?;
            format!("[{}]{text}", timestamp(time)?)
        } else {
            text.into()
        });
    }
    let text = output.join("\n");
    if !synchronized {
        return Ok(format!("{text}\n"));
    }
    let offset = offset_ms + metadata.offset_ms.unwrap_or_default();
    ensure!(offset.is_finite(), "Lyric export offset must be finite");
    Ok(format!(
        "[ar:{}]\n[ti:{}]\n[offset:{offset}]\n{text}\n",
        metadata.artist, metadata.name
    ))
}

fn timestamp(milliseconds: f64) -> Result<String> {
    ensure!(
        milliseconds.is_finite() && milliseconds.abs() < u64::MAX as f64,
        "Invalid lyric timestamp"
    );
    let value = milliseconds.abs().trunc() as u64;
    let days = value / 86_400_000;
    let hours = value / 3_600_000 % 24;
    let minutes = value / 60_000 % 60;
    let seconds = value / 1000 % 60;
    let sign = if milliseconds < 0. { "-" } else { "" };
    let clock = if days > 0 {
        format!("{days}:{hours:02}:{minutes:02}:{seconds:02}")
    } else if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    };
    Ok(format!("{sign}{clock}.{:03}", value % 1000))
}

#[cfg(test)]
mod tests {
    #[test]
    fn export_matches_original_metadata_formatter() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyric-export-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let metadata = serde_json::from_value(case["metadata"].clone()).unwrap();
            assert_eq!(
                super::render(
                    &metadata,
                    case["offset"].as_f64().unwrap(),
                    case["synced"].as_bool().unwrap()
                )
                .unwrap(),
                case["output"].as_str().unwrap(),
                "{case}"
            );
        }
    }
}
