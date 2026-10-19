use crate::{
    lyrics::{self, Line},
    model::Item,
    storage,
};
use anyhow::{Context, Result, ensure};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Provider {
    Lrclib,
    NetEase,
    Genius,
    SimpMusic,
}
impl Provider {
    pub const ALL: [Self; 4] = [Self::Lrclib, Self::NetEase, Self::Genius, Self::SimpMusic];
    pub fn name(self) -> &'static str {
        match self {
            Self::Lrclib => "LRCLIB",
            Self::NetEase => "NetEase",
            Self::Genius => "Genius",
            Self::SimpMusic => "SimpMusic",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Match {
    pub provider: Provider,
    pub id: String,
    pub name: String,
    pub artist: String,
    pub synchronized: bool,
    #[serde(default)]
    pub score: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub raw: String,
    pub matched: Option<Match>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<lyrics::Variant>,
    #[serde(default)]
    pub selected: usize,
}
impl Record {
    pub fn lines(&self) -> Vec<Line> {
        self.variants
            .get(self.selected)
            .map(|variant| variant.lines.clone())
            .unwrap_or_else(|| lyrics::display_lines(&self.raw))
    }
}

pub struct Sources {
    client: Client,
    bases: HashMap<Provider, String>,
}
impl Sources {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .connect_timeout(Duration::from_secs(5))
                .user_agent("Feishin/1.17.0 (https://github.com/jeffvli/feishin)")
                .build()?,
            bases: [
                (Provider::Lrclib, "https://lrclib.net/api"),
                (Provider::NetEase, "https://music.163.com/api"),
                (Provider::Genius, "https://genius.com/api"),
                (Provider::SimpMusic, "https://api-lyrics.simpmusic.org/v1"),
            ]
            .into_iter()
            .map(|(p, url)| (p, url.into()))
            .collect(),
        })
    }
    fn bytes(&self, url: &str, params: &[(&str, String)]) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .query(params)
            .send()
            .map_err(|e| e.without_url())?
            .error_for_status()
            .map_err(|e| e.without_url())?;
        let mut bytes = Vec::new();
        response.take(2_000_001).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 2_000_000,
            "Lyric provider response is too large"
        );
        Ok(bytes)
    }
    fn json(&self, provider: Provider, path: &str, params: &[(&str, String)]) -> Result<Value> {
        serde_json::from_slice(&self.bytes(&format!("{}{path}", self.bases[&provider]), params)?)
            .context("Lyric provider returned invalid JSON")
    }
    pub fn search(&self, item: &Item, providers: &[Provider]) -> Result<Vec<Match>> {
        ensure!(
            !item.name.trim().is_empty() || !item.artist.trim().is_empty(),
            "Enter a song or artist to search lyrics"
        );
        let batches = std::thread::scope(|scope| {
            providers
                .iter()
                .map(|provider| scope.spawn(move || self.search_one(item, *provider)))
                .collect::<Vec<_>>()
                .into_iter()
                .map(|task| {
                    task.join()
                        .unwrap_or_else(|_| Err(anyhow::anyhow!("Lyric search worker failed")))
                })
                .collect::<Vec<_>>()
        });
        let mut results = vec![];
        let mut errors = vec![];
        let mut successes = 0;
        for batch in batches {
            match batch {
                Ok(mut items) => {
                    successes += 1;
                    results.append(&mut items);
                }
                Err(error) => {
                    log::debug!("Lyric search failed: {error}");
                    errors.push(error.to_string());
                }
            }
        }
        ensure!(
            successes > 0 || providers.is_empty(),
            "{}",
            errors.join("; ")
        );
        // Keep provider batches distinct for the manual picker. Provider-local
        // identities can collide, and automatic ranking deduplicates by identity.
        Ok(results)
    }
    fn search_one(&self, item: &Item, provider: Provider) -> Result<Vec<Match>> {
        let query = format!("{} {}", item.primary_artist(), item.name);
        let value = match provider {
            Provider::Lrclib => self.json(
                provider,
                "/search",
                &[("q", format!("{} {}", item.name, item.primary_artist()))],
            )?,
            Provider::SimpMusic => self.json(provider, "/search", &[("q", item.name.clone())])?,
            Provider::NetEase => self.json(
                provider,
                "/search/get",
                &[
                    ("s", query),
                    ("limit", "5".into()),
                    ("offset", "0".into()),
                    ("type", "1".into()),
                ],
            )?,
            Provider::Genius => self.json(
                provider,
                "/search/song",
                &[("q", query), ("per_page", "5".into())],
            )?,
        };
        Ok(crate::lyric_match::order(
            parse_matches(provider, &value),
            &item.name,
            item.primary_artist(),
        ))
    }
    pub fn get(&self, matched: &Match, translation: bool) -> Result<Record> {
        let raw = if matched.provider == Provider::Genius {
            let url = url::Url::parse(&matched.id)?;
            ensure!(
                url.scheme() == "https"
                    && url.host_str() == Some("genius.com")
                    && url.username().is_empty()
                    && url.password().is_none(),
                "Invalid Genius lyric URL"
            );
            genius_text(&String::from_utf8(self.bytes(url.as_str(), &[])?)?)
        } else {
            ensure!(
                !matched.id.is_empty()
                    && matched
                        .id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "Invalid lyric identity"
            );
            let value = match matched.provider {
                Provider::Lrclib => {
                    self.json(matched.provider, &format!("/get/{}", matched.id), &[])?
                }
                Provider::SimpMusic => {
                    self.json(matched.provider, &format!("/{}", matched.id), &[])?
                }
                Provider::NetEase => self.json(
                    matched.provider,
                    "/song/lyric",
                    &[
                        ("id", matched.id.clone()),
                        ("kv", "-1".into()),
                        ("lv", "-1".into()),
                        ("tv", "-1".into()),
                    ],
                )?,
                _ => unreachable!(),
            };
            if matched.provider == Provider::NetEase && translation {
                lyrics::merge_translation(
                    text(&value["lrc"], "lyric"),
                    text(&value["tlyric"], "lyric"),
                )
            } else {
                provider_text(matched.provider, &value)
            }
        };
        ensure!(
            !lyrics::display_lines(&raw).is_empty(),
            "This result contains no lyrics"
        );
        Ok(Record {
            raw,
            matched: Some(matched.clone()),
            variants: vec![],
            selected: 0,
        })
    }
    pub fn automatic(
        &self,
        item: &Item,
        providers: &[Provider],
        translation: bool,
    ) -> Result<Option<Record>> {
        let Some(matched) = crate::lyric_match::order(
            self.search(item, providers)?,
            &item.name,
            item.primary_artist(),
        )
        .into_iter()
        .next()
        .filter(|result| result.score <= 0.55) else {
            return Ok(None);
        };
        match self.get(&matched, translation) {
            Ok(record) => Ok(Some(record)),
            Err(error) => {
                log::debug!("Lyric fetch failed: {error}");
                Ok(None)
            }
        }
    }
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}
fn identity(v: &Value, key: &str) -> String {
    v[key]
        .as_str()
        .map(str::to_owned)
        .or_else(|| v[key].as_u64().map(|n| n.to_string()))
        .unwrap_or_default()
}
fn parse_matches(provider: Provider, value: &Value) -> Vec<Match> {
    let rows = match provider {
        Provider::Lrclib => value,
        Provider::NetEase => &value["result"]["songs"],
        Provider::Genius => &value["response"]["sections"][0]["hits"],
        Provider::SimpMusic => &value["data"],
    };
    rows.as_array()
        .map(|rows| {
            rows.iter()
                .take(100)
                .filter_map(|row| {
                    let row = if provider == Provider::Genius {
                        &row["result"]
                    } else {
                        row
                    };
                    let (id, name, artist) = match provider {
                        Provider::Lrclib => (
                            identity(row, "id"),
                            row["name"]
                                .as_str()
                                .unwrap_or_else(|| text(row, "trackName"))
                                .into(),
                            text(row, "artistName").into(),
                        ),
                        Provider::SimpMusic => (
                            identity(row, "videoId"),
                            text(row, "songTitle").into(),
                            text(row, "artistName").into(),
                        ),
                        Provider::NetEase => (
                            identity(row, "id"),
                            text(row, "name").into(),
                            row["artists"]
                                .as_array()
                                .map(|artists| {
                                    artists
                                        .iter()
                                        .take(32)
                                        .map(|artist| text(artist, "name"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default(),
                        ),
                        Provider::Genius => (
                            identity(row, "url"),
                            row["full_title"]
                                .as_str()
                                .unwrap_or_else(|| text(row, "title"))
                                .into(),
                            text(row, "artist_names").into(),
                        ),
                    };
                    (!id.is_empty()).then_some(Match {
                        provider,
                        id,
                        name,
                        artist,
                        synchronized: matches!(provider, Provider::Lrclib | Provider::SimpMusic)
                            && !text(row, "syncedLyrics").is_empty(),
                        score: 0.0,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
fn provider_text(provider: Provider, value: &Value) -> String {
    let value = if provider == Provider::SimpMusic {
        value["data"][0]
            .as_object()
            .map(|_| &value["data"][0])
            .unwrap_or(value)
    } else {
        value
    };
    match provider {
        Provider::NetEase => text(&value["lrc"], "lyric").into(),
        _ => {
            let synced = text(value, "syncedLyrics");
            if synced.is_empty() {
                text(
                    value,
                    if provider == Provider::SimpMusic {
                        "plainLyric"
                    } else {
                        "plainLyrics"
                    },
                )
                .into()
            } else {
                synced.into()
            }
        }
    }
}
/// Preserve server biography formatting for the native text renderer. The
/// renderer separately restricts links and images to safe URL schemes.
pub fn formatted_html(input: &str) -> String {
    use html5ever::{parse_document, serialize, tendril::TendrilSink};
    use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};
    fn prune(node: &Handle, depth: usize) {
        let mut children = node.children.borrow_mut();
        if depth >= 128 {
            children.clear();
            return;
        }
        children.retain(|child| {
            !matches!(&child.data, NodeData::Element { name, .. } if matches!(
                name.local.as_ref(),
                "head" | "script" | "style" | "iframe" | "object" | "embed" | "template" | "noscript"
            ))
        });
        for child in children.iter() {
            if let NodeData::Element { attrs, .. } = &child.data {
                attrs
                    .borrow_mut()
                    .retain(|attr| !attr.name.local.as_ref().starts_with("on"));
            }
            prune(child, depth + 1);
        }
    }
    if input.trim().is_empty() {
        return String::new();
    }
    let input = input.chars().take(1_000_000).collect::<String>();
    let dom = parse_document(RcDom::default(), Default::default()).one(input);
    prune(&dom.document, 0);
    let mut output = Vec::new();
    if serialize(
        &mut output,
        &SerializableHandle::from(dom.document),
        Default::default(),
    )
    .is_err()
    {
        return String::new();
    }
    String::from_utf8(output).unwrap_or_default()
}

pub fn text_from_html(input: &str) -> String {
    use html5ever::{parse_document, tendril::TendrilSink};
    use markup5ever_rcdom::{Handle, NodeData, RcDom};
    fn visit(node: &Handle, text: &mut String, depth: usize) {
        if depth > 128 {
            return;
        }
        let block = match &node.data {
            NodeData::Text { contents } => {
                text.push_str(&contents.borrow());
                false
            }
            NodeData::Element { name, .. } => {
                let name = name.local.as_ref();
                if matches!(name, "script" | "style" | "head") {
                    return;
                }
                matches!(name, "br" | "p" | "div" | "li")
            }
            _ => false,
        };
        if block && !text.ends_with('\n') {
            text.push('\n');
        }
        for child in node.children.borrow().iter() {
            visit(child, text, depth + 1);
        }
        if block && !text.ends_with('\n') {
            text.push('\n');
        }
    }
    let input = input.chars().take(1_000_000).collect::<String>();
    let dom = parse_document(RcDom::default(), Default::default()).one(input);
    let mut text = String::new();
    visit(&dom.document, &mut text, 0);
    text.trim().into()
}
fn genius_text(input: &str) -> String {
    use html5ever::{parse_document, tendril::TendrilSink};
    use markup5ever_rcdom::{Handle, NodeData, RcDom};
    fn attribute(handle: &Handle, key: &str) -> Option<String> {
        if let NodeData::Element { attrs, .. } = &handle.data {
            attrs
                .borrow()
                .iter()
                .find(|attr| attr.name.local.as_ref() == key)
                .map(|attr| attr.value.to_string())
        } else {
            None
        }
    }
    fn content(node: &Handle, out: &mut String, depth: usize) {
        if depth > 128 || attribute(node, "data-exclude-from-selection").as_deref() == Some("true")
        {
            return;
        }
        match &node.data {
            NodeData::Text { contents } => out.push_str(&contents.borrow()),
            NodeData::Element { name, .. } if name.local.as_ref() == "br" => out.push('\n'),
            _ => {}
        }
        for child in node.children.borrow().iter() {
            content(child, out, depth + 1);
        }
    }
    fn containers(node: &Handle, old: &mut Vec<String>, modern: &mut Vec<String>, depth: usize) {
        if depth > 128 {
            return;
        }
        if attribute(node, "class")
            .is_some_and(|class| class.split_whitespace().any(|word| word == "lyrics"))
        {
            let mut text = String::new();
            content(node, &mut text, 0);
            old.push(text);
            return;
        }
        if attribute(node, "data-lyrics-container").as_deref() == Some("true") {
            let mut text = String::new();
            content(node, &mut text, 0);
            modern.push(text);
            return;
        }
        for child in node.children.borrow().iter() {
            containers(child, old, modern, depth + 1);
        }
    }
    let dom = parse_document(RcDom::default(), Default::default()).one(input);
    let (mut old, mut modern) = (vec![], vec![]);
    containers(&dom.document, &mut old, &mut modern, 0);
    if old.is_empty() {
        modern.join("\n").trim().into()
    } else {
        old.join("\n").trim().into()
    }
}

pub fn cache_path(root: &Path, item: &Item, override_lyrics: bool) -> PathBuf {
    root.join("lyrics")
        .join(if override_lyrics {
            "overrides"
        } else {
            "cache"
        })
        .join(format!(
            "{:x}.json",
            md5::compute(format!("{}:{}", item.server_id, item.id))
        ))
}
pub fn cache_path_revision(root: &Path, item: &Item, revision: u64) -> PathBuf {
    let path = cache_path(root, item, false);
    path.with_file_name(format!(
        "{}.json",
        crate::cache::versioned(&path.file_stem().unwrap().to_string_lossy(), revision)
    ))
}
pub fn read_record(path: &Path) -> Result<Option<Record>> {
    match std::fs::File::open(path) {
        Ok(file) => {
            let mut bytes = vec![];
            file.take(2_000_001).read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 2_000_000, "Saved lyrics are too large");
            let record: Record = serde_json::from_slice(&bytes)?;
            ensure!(
                record.variants.is_empty() || record.selected < record.variants.len(),
                "Invalid saved lyric selection"
            );
            Ok(Some(record))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn save_record(path: &Path, record: &Record) -> Result<()> {
    ensure!(record.raw.len() <= 1_000_000, "Lyrics are too large");
    storage::atomic_write(path, &serde_json::to_vec(record)?)
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoadOptions {
    pub providers: Vec<Provider>,
    pub translation: bool,
    pub fetch: bool,
    pub prefer_local: bool,
}
#[derive(Serialize, Deserialize)]
struct Cache {
    options: LoadOptions,
    record: Record,
}

pub fn load(
    root: &Path,
    item: &Item,
    options: &LoadOptions,
    refresh: bool,
    server: impl FnOnce() -> Result<Vec<lyrics::Variant>>,
) -> Result<Option<Record>> {
    load_from(root, item, options, refresh, server, || {
        Sources::new()?.automatic(item, &options.providers, options.translation)
    })
}
pub fn load_with_cache_revision(
    root: &Path,
    item: &Item,
    options: &LoadOptions,
    refresh: bool,
    revision: u64,
    server: impl FnOnce() -> Result<Vec<lyrics::Variant>>,
) -> Result<Option<Record>> {
    load_from_revision(root, item, options, refresh, revision, server, || {
        Sources::new()?.automatic(item, &options.providers, options.translation)
    })
}
fn load_from(
    root: &Path,
    item: &Item,
    options: &LoadOptions,
    refresh: bool,
    server: impl FnOnce() -> Result<Vec<lyrics::Variant>>,
    remote: impl FnOnce() -> Result<Option<Record>>,
) -> Result<Option<Record>> {
    load_from_revision(root, item, options, refresh, 0, server, remote)
}
fn load_from_revision(
    root: &Path,
    item: &Item,
    options: &LoadOptions,
    refresh: bool,
    revision: u64,
    server: impl FnOnce() -> Result<Vec<lyrics::Variant>>,
    remote: impl FnOnce() -> Result<Option<Record>>,
) -> Result<Option<Record>> {
    if let Some(record) = read_record(&cache_path(root, item, true))? {
        return Ok(Some(record));
    }
    let path = cache_path_revision(root, item, revision);
    if !refresh && let Ok(file) = std::fs::File::open(&path) {
        let mut bytes = vec![];
        file.take(2_000_001).read_to_end(&mut bytes)?;
        if bytes.len() <= 2_000_000
            && let Ok(cache) = serde_json::from_slice::<Cache>(&bytes)
            && cache.options == *options
            && (cache.record.variants.is_empty()
                || cache.record.selected < cache.record.variants.len())
        {
            return Ok(Some(cache.record));
        }
    }
    let server_record = || -> Option<Record> {
        match server() {
            Ok(mut variants) => {
                variants.retain(|variant| !variant.lines.is_empty());
                if variants.is_empty() {
                    return None;
                }
                let selected = variants
                    .iter()
                    .position(|variant| variant.synchronized && variant.kind == "main")
                    .unwrap_or(0);
                Some(Record {
                    raw: lyrics::render_lrc(&variants[selected].lines),
                    matched: None,
                    variants,
                    selected,
                })
            }
            Err(error) => {
                log::debug!("Server lyrics unavailable: {error}");
                None
            }
        }
    };
    let remote_record = || -> Option<Record> {
        if !options.fetch || options.providers.is_empty() {
            return None;
        }
        match remote() {
            Ok(record) => record,
            Err(error) => {
                log::debug!("External lyrics unavailable: {error}");
                None
            }
        }
    };
    let record = if options.prefer_local {
        server_record().or_else(remote_record)
    } else {
        remote_record().or_else(server_record)
    };
    if let Some(record) = &record
        && let Err(error) = storage::atomic_write(
            &path,
            &serde_json::to_vec(&Cache {
                options: options.clone(),
                record: record.clone(),
            })?,
        )
    {
        log::warn!("Could not cache lyrics: {error}");
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn cache_revisions_skip_obsolete_provider_results_and_keep_imported_lyrics() {
        let root =
            std::env::temp_dir().join(format!("feishin-lyric-revision-{}", uuid::Uuid::new_v4()));
        let item = Item::default();
        let options = LoadOptions {
            providers: vec![],
            fetch: false,
            prefer_local: true,
            translation: false,
        };
        let variant = |text: &str| {
            vec![lyrics::Variant {
                lines: lyrics::parse_lrc(text),
                ..Default::default()
            }]
        };
        let first = load_with_cache_revision(&root, &item, &options, false, 0, || {
            Ok(variant("old server lyrics"))
        })
        .unwrap()
        .unwrap();
        assert!(first.raw.contains("old server lyrics"));
        let cached = load_with_cache_revision(&root, &item, &options, false, 0, || {
            panic!("Must use this revision's cache")
        })
        .unwrap()
        .unwrap();
        assert_eq!(cached.raw, first.raw);
        let next = load_with_cache_revision(&root, &item, &options, false, 1, || {
            Ok(variant("new server lyrics"))
        })
        .unwrap()
        .unwrap();
        assert!(next.raw.contains("new server lyrics"));
        assert_ne!(
            cache_path_revision(&root, &item, 0),
            cache_path_revision(&root, &item, 1)
        );
        let imported = Record {
            raw: "[00:01]Imported lyrics".into(),
            matched: None,
            variants: vec![],
            selected: 0,
        };
        save_record(&cache_path(&root, &item, true), &imported).unwrap();
        crate::cache::clear_disk(&root, &[]).unwrap();
        let retained = load_with_cache_revision(&root, &item, &options, true, 2, || {
            panic!("Imported lyrics must win")
        })
        .unwrap()
        .unwrap();
        assert_eq!(retained.raw, imported.raw);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fetch_preferences_cache_policy_and_failed_sources_keep_the_correct_lyrics() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let item = Item::default();
        let local = || {
            Ok(vec![lyrics::Variant {
                language: "eng".into(),
                kind: "main".into(),
                synchronized: true,
                offset: Some(0.25),
                lines: lyrics::parse_lrc("[00:01]Local"),
                agents: vec![],
            }])
        };
        let remote = || {
            Ok(Some(Record {
                raw: "[00:01]Remote".into(),
                matched: Some(Match {
                    provider: Provider::NetEase,
                    id: "1".into(),
                    name: "Song".into(),
                    artist: "Artist".into(),
                    synchronized: true,
                    score: 0.0,
                }),
                variants: vec![],
                selected: 0,
            }))
        };
        let mut options = LoadOptions {
            providers: vec![Provider::NetEase],
            translation: false,
            fetch: true,
            prefer_local: true,
        };
        let result = load_from(&root, &item, &options, false, local, || {
            panic!("local preference should use local lyrics immediately")
        })
        .unwrap()
        .unwrap();
        assert_eq!(result.lines()[0].text, "Local");
        assert_eq!(result.variants[0].offset, Some(0.25));
        assert_eq!(
            load_from(
                &root,
                &item,
                &options,
                false,
                || panic!("cached"),
                || panic!("cached")
            )
            .unwrap()
            .unwrap()
            .lines()[0]
                .text,
            "Local"
        );
        options.prefer_local = false;
        assert_eq!(
            load_from(
                &root,
                &item,
                &options,
                false,
                || panic!("remote preference should use remote lyrics"),
                remote
            )
            .unwrap()
            .unwrap()
            .lines()[0]
                .text,
            "Remote"
        );
        options.translation = true;
        let result = load_from(&root, &item, &options, false, local, || {
            let mut record = remote()?.unwrap();
            record.raw = "[00:01]Translation".into();
            Ok(Some(record))
        })
        .unwrap()
        .unwrap();
        assert!(result.raw.contains("Translation"));
        options.fetch = false;
        assert_eq!(
            load_from(&root, &item, &options, false, local, || panic!(
                "fetch disabled"
            ))
            .unwrap()
            .unwrap()
            .lines()[0]
                .text,
            "Local"
        );
        options.fetch = true;
        options.prefer_local = true;
        assert_eq!(
            load_from(
                &root,
                &item,
                &options,
                true,
                || anyhow::bail!("server offline"),
                remote
            )
            .unwrap()
            .unwrap()
            .lines()[0]
                .text,
            "Remote"
        );
        options.prefer_local = false;
        assert_eq!(
            load_from(&root, &item, &options, true, local, || anyhow::bail!(
                "provider offline"
            ))
            .unwrap()
            .unwrap()
            .lines()[0]
                .text,
            "Local"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn provider_http_search_fetch_translation_and_automatic_metadata_contracts() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..10 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = vec![];
                let mut buffer = [0; 4096];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                let request = String::from_utf8(request).unwrap();
                let target = request.split_whitespace().nth(1).unwrap();
                let url = url::Url::parse(&format!("http://fixture{target}")).unwrap();
                let response = match url.path() {
                    "/Lrclib/search" => json!([{"id":4,"name":"Song & Dance","artistName":"A / B","syncedLyrics":"[00:01]Original"}]),
                    "/NetEase/search/get" => json!({"result":{"songs":[{"id":4,"name":"Song & Dance","artists":[{"name":"A / B"}]}]}}),
                    "/Genius/search/song" => json!({"response":{"sections":[{"hits":[{"result":{"url":"https://genius.com/test-lyrics","title":"Song & Dance","artist_names":"A / B"}}]}]}}),
                    "/SimpMusic/search" => json!({"data":[{"videoId":"ab-12","songTitle":"Song & Dance","artistName":"A / B"}]}),
                    "/Lrclib/get/4" => json!({"id":4,"name":"Song & Dance","artistName":"A / B","syncedLyrics":"[00:01]Original"}),
                    "/NetEase/song/lyric" => { assert!(url.query_pairs().any(|(key,value)| key == "id" && value == "4")); json!({"lrc":{"lyric":"[00:01]Original"},"tlyric":{"lyric":"[00:01]Translated"}}) },
                    "/SimpMusic/ab-12" => json!({"data":[{"syncedLyrics":"[00:01]Simp"}]}),
                    path => panic!("Unexpected lyric provider path {path}"),
                }.to_string();
                if url.path().ends_with("/search") && url.path().starts_with("/Lrclib") {
                    assert!(
                        url.query_pairs()
                            .any(|(key, value)| key == "q" && value == "Song & Dance A / B")
                    );
                }
                requests.push(request);
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).unwrap();
            }
            requests
        });
        let mut sources = Sources::new().unwrap();
        for provider in Provider::ALL {
            sources
                .bases
                .insert(provider, format!("{base}/{provider:?}"));
        }
        let item = Item {
            name: "Song & Dance".into(),
            artist: "A / B, Guest".into(),
            artist_names: vec!["A / B".into(), "Guest".into()],
            duration: 90.0,
            ..Default::default()
        };
        let providers = [
            Provider::NetEase,
            Provider::Lrclib,
            Provider::SimpMusic,
            Provider::Genius,
        ];
        let matched = sources.search(&item, &providers).unwrap();
        assert_eq!(matched.len(), 4);
        assert_eq!(
            matched.iter().map(|row| row.provider).collect::<Vec<_>>(),
            providers
        );
        assert_eq!(matched[0].id, matched[1].id);
        for provider in [Provider::Lrclib, Provider::NetEase, Provider::SimpMusic] {
            let record = sources
                .get(
                    matched
                        .iter()
                        .find(|matched| matched.provider == provider)
                        .unwrap(),
                    true,
                )
                .unwrap();
            assert_eq!(record.lines()[0].time, Some(1.0));
            if provider == Provider::NetEase {
                assert!(record.raw.contains("Translated"));
            }
        }
        let automatic = sources
            .automatic(&item, &[Provider::NetEase, Provider::Lrclib], false)
            .unwrap()
            .unwrap();
        assert!(automatic.raw.contains("Original"));
        assert_eq!(automatic.matched.unwrap().provider, Provider::Lrclib);
        assert_eq!(worker.join().unwrap().len(), 10);
        assert!(
            sources
                .get(
                    &Match {
                        provider: Provider::Lrclib,
                        id: "../invalid".into(),
                        name: String::new(),
                        artist: String::new(),
                        synchronized: false,
                        score: 0.0
                    },
                    false
                )
                .is_err()
        );
    }
    #[test]
    fn provider_contracts_html_and_persistent_overrides() {
        let biography = formatted_html(
            "<p onclick='alert(1)'><strong>A &amp; B</strong><script>hidden</script><style>hidden</style><iframe>hidden</iframe></p><ul><li><a href='https://example.com/artist'><em>Artist</em></a></li></ul><template>hidden</template>",
        );
        assert!(biography.contains("<strong>A &amp; B</strong>"));
        assert!(biography.contains(
            "<ul><li><a href=\"https://example.com/artist\"><em>Artist</em></a></li></ul>"
        ));
        assert!(
            !biography.contains("hidden") && !biography.contains("onclick"),
            "{biography}"
        );
        assert_eq!(text_from_html(&biography), "A & B\nArtist");
        assert_eq!(formatted_html("  "), "");
        let lrclib =
            json!([{"id":4,"name":"Song","artistName":"Artist","syncedLyrics":"[00:01]Hello"}]);
        let matched = parse_matches(Provider::Lrclib, &lrclib);
        assert_eq!(matched[0].id, "4");
        assert!(matched[0].synchronized);
        assert_eq!(
            parse_matches(
                Provider::NetEase,
                &json!({"result":{"songs":[{"id":8,"name":"Song","artists":[{"name":"Artist"}]}]}})
            )[0]
            .artist,
            "Artist"
        );
        assert_eq!(parse_matches(Provider::Genius,&json!({"response":{"sections":[{"hits":[{"result":{"url":"https://genius.com/test-lyrics","title":"Song","artist_names":"Artist"}}]}]}}))[0].name,"Song");
        let simp = json!({"data":[{"videoId":"ab-12","songTitle":"Song","artistName":"Artist","plainLyric":"Hello"}]});
        assert_eq!(parse_matches(Provider::SimpMusic, &simp)[0].id, "ab-12");
        assert_eq!(provider_text(Provider::SimpMusic, &simp), "Hello");
        assert_eq!(
            genius_text(
                "<div data-lyrics-container='true'>Hello &amp; <a>world</a><br><span data-exclude-from-selection='true'>Ads</span>Second</div><div data-lyrics-container='true'>Third</div>"
            ),
            "Hello & world\nSecond\nThird"
        );
        assert_eq!(
            genius_text("<div class='lyrics'><p>Hello<br/>world</p></div>"),
            "Hello\nworld"
        );
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let item = Item {
            id: "../../song".into(),
            ..Default::default()
        };
        let record = Record {
            raw: "[00:01]Override".into(),
            matched: None,
            variants: vec![],
            selected: 0,
        };
        let path = cache_path(&root, &item, true);
        save_record(&path, &record).unwrap();
        assert_eq!(
            load(
                &root,
                &item,
                &LoadOptions {
                    providers: vec![],
                    translation: false,
                    fetch: false,
                    prefer_local: true
                },
                false,
                || panic!("override must take priority")
            )
            .unwrap()
            .unwrap()
            .raw,
            record.raw
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
