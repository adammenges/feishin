//! Bounded, redacted native logs and a portable diagnostics archive.
use crate::storage::{self, State};
use anyhow::Result;
use regex::Regex;
use serde_json::{Value, json};
use std::{
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
const LIMIT: u64 = 5_000_000;
static DIRECTORY: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
static URLS: OnceLock<Regex> = OnceLock::new();
static KEYS: OnceLock<Regex> = OnceLock::new();
static BEARER: OnceLock<Regex> = OnceLock::new();
fn sensitive(key: &str) -> bool {
    let key = key.to_lowercase();
    [
        "password",
        "token",
        "credential",
        "authorization",
        "secret",
        "cookie",
        "apikey",
        "api_key",
        "username",
    ]
    .iter()
    .any(|word| key.contains(word))
}
pub fn text(value: &str) -> String {
    let urls = URLS.get_or_init(|| Regex::new(r#"https?://[^\s<>"']+"#).unwrap());
    let value = urls.replace_all(value, |capture: &regex::Captures| {
        url::Url::parse(&capture[0])
            .map(|url| format!("{}/[Redacted]", url.origin().ascii_serialization()))
            .unwrap_or_else(|_| "[Redacted URL]".into())
    });
    let bearer = BEARER.get_or_init(|| Regex::new(r#"(?i)Bearer\s+[^\s,;"'}]+"#).unwrap());
    let value = bearer.replace_all(&value, "Bearer [Redacted]");
    let keys = KEYS.get_or_init(|| Regex::new(r#"(?i)((?:password|token|credential|authorization|secret|cookie|api[_-]?key)\s*["']?\s*[:=]\s*)(?:"[^"]*"|'[^']*'|[^\s,;"'}]+)"#).unwrap());
    keys.replace_all(&value, "${1}[Redacted]").into_owned()
}
pub fn sanitize(value: &Value) -> Value {
    sanitize_at(value, 0)
}
fn sanitize_at(value: &Value, depth: usize) -> Value {
    if depth >= 20 {
        return "[Truncated]".into();
    }
    match value {
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        if sensitive(key) {
                            "[Redacted]".into()
                        } else {
                            sanitize_at(value, depth + 1)
                        },
                    )
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| sanitize_at(value, depth + 1))
                .collect(),
        ),
        Value::String(value) => text(value).into(),
        other => other.clone(),
    }
}
pub fn configure(data_dir: &Path) -> Result<PathBuf> {
    let dir = data_dir.join("logs");
    std::fs::create_dir_all(&dir)?;
    *DIRECTORY.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some(dir.clone());
    Ok(dir)
}
pub fn record(level: log::Level, target: &str, message: &str) -> String {
    let message = text(message)
        .chars()
        .take(16_384)
        .collect::<String>()
        .replace('\n', "\\n")
        .replace('\r', "");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let line = format!("{timestamp} {level} [{target}] {message}\n");
    if let Some(directory) = DIRECTORY.get()
        && let Ok(directory) = directory.lock()
        && let Some(directory) = directory.as_ref()
    {
        let path = directory.join("feishin.log");
        if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() >= LIMIT) {
            let previous = directory.join("feishin.previous.log");
            let _ = std::fs::remove_file(&previous);
            let _ = std::fs::rename(&path, previous);
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        if let Ok(mut file) = options.open(path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
    line
}
pub fn archive(
    state: &State,
    data_dir: &Path,
    playback: Value,
    servers: Vec<Value>,
) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    let metadata = json!({"app":{"name":"Feishin","version":env!("CARGO_PKG_VERSION"),"ui":"GPUI 0.2.2"}, "platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"exportedAt":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),"locale":state.settings.language,"playback":playback,"servers":servers,"queue":{"count":state.queue.entries.len(),"position":state.queue.position,"currentIndex":state.queue.order.iter().position(|key| Some(*key) == state.queue.current)}});
    for (name, value) in [
        ("diagnostics.json", metadata),
        ("settings.json", serde_json::to_value(&state.settings)?),
    ] {
        writer.start_file(name, options)?;
        writer.write_all(&serde_json::to_vec_pretty(&sanitize(&value))?)?;
    }
    for name in ["feishin.log", "feishin.previous.log"] {
        let path = data_dir.join("logs").join(name);
        if let Ok(mut file) = std::fs::File::open(path) {
            use std::io::Read;
            let mut bytes = vec![];
            std::io::Read::by_ref(&mut file)
                .take(LIMIT + 16_384)
                .read_to_end(&mut bytes)?;
            let log = text(&String::from_utf8_lossy(&bytes));
            writer.start_file(format!("logs/{name}"), options)?;
            writer.write_all(log.as_bytes())?;
        }
    }
    Ok(writer.finish()?.into_inner())
}
pub fn export(
    state: &State,
    data_dir: &Path,
    path: &Path,
    playback: Value,
    servers: Vec<Value>,
) -> Result<()> {
    storage::atomic_write(path, &archive(state, data_dir, playback, servers)?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_zip_redacts_config_and_logs_without_exporting_queue_urls() {
        let dir =
            std::env::temp_dir().join(format!("feishin-diagnostics-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("logs")).unwrap();
        std::fs::write(dir.join("logs/feishin.log"), "request https://user:password@localhost/secret-path?token=url-secret Authorization: Bearer auth-secret token=inline-secret\n").unwrap();
        let mut state = State::default();
        state.settings.mpv_properties.insert(
            "http-header-fields".into(),
            "Authorization: Bearer setting-secret".into(),
        );
        state.queue.add(
            vec![crate::model::Item {
                stream_url: "https://hidden-radio?token=queue-secret".into(),
                ..Default::default()
            }],
            crate::queue::Enqueue::Now,
        );
        let bytes = archive(&state, &dir, json!({"engine":"native"}), vec![json!({"type":"Navidrome","password":"server-secret","url":"https://host/path?apiKey=api-secret"})]).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 3);
        let mut combined = String::new();
        for index in 0..archive.len() {
            use std::io::Read;
            archive
                .by_index(index)
                .unwrap()
                .read_to_string(&mut combined)
                .unwrap();
        }
        for secret in [
            "password@",
            "secret-path",
            "url-secret",
            "auth-secret",
            "inline-secret",
            "setting-secret",
            "queue-secret",
            "server-secret",
            "api-secret",
        ] {
            assert!(!combined.contains(secret), "Leaked {secret}");
        }
        assert!(combined.contains("[Redacted]") && combined.contains("native"));
        assert_eq!(text("password=\"two words secret\""), "password=[Redacted]");
        assert_eq!(
            sanitize(&json!({"nested":{"apiToken":"secret"}}))["nested"]["apiToken"],
            "[Redacted]"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
