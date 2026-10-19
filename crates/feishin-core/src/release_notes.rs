use anyhow::{Context, Result, ensure};
use reqwest::blocking::Client as HttpClient;
use serde::Deserialize;
use std::{io::Read, time::Duration};
use url::Url;

const API: &str = "https://api.github.com/repos/jeffvli/feishin/";
const WEB: &str = "https://github.com/jeffvli/feishin/";
pub const HISTORY_LIMIT: usize = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub tag: String,
    pub published_at: String,
    pub prerelease: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub page: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Document {
    Markdown {
        body: String,
        page: String,
    },
    Commits {
        stable: String,
        commits: Vec<Commit>,
        total: usize,
        page: String,
    },
    NoStable {
        page: String,
    },
}

impl Document {
    pub fn page(&self) -> &str {
        match self {
            Self::Markdown { page, .. } | Self::Commits { page, .. } | Self::NoStable { page } => {
                page
            }
        }
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

#[derive(Deserialize)]
struct ApiComparison {
    commits: Vec<ApiCommit>,
    total_commits: usize,
}

#[derive(Deserialize)]
struct ApiCommit {
    sha: String,
    html_url: String,
    commit: ApiCommitDetails,
}

#[derive(Deserialize)]
struct ApiCommitDetails {
    message: String,
    author: ApiAuthor,
}

#[derive(Deserialize)]
struct ApiAuthor {
    name: String,
    date: String,
}

pub fn tag(version: &str) -> Result<String> {
    let version = version.strip_prefix('v').unwrap_or(version);
    ensure!(version.len() <= 256, "Release version is too long");
    semver::Version::parse(version).context("Invalid release version")?;
    Ok(format!("v{version}"))
}

pub fn release_page(version: &str) -> Result<String> {
    Ok(format!("{WEB}releases/tag/{}", tag(version)?))
}

pub fn fallback_page(version: &str, history: &[Release]) -> Result<String> {
    let head = tag(version)?;
    if head.contains("-alpha")
        && let Some(stable) = history.iter().find(|release| !release.prerelease)
    {
        return Ok(format!("{WEB}compare/{}...{head}", tag(&stable.tag)?));
    }
    release_page(version)
}

pub struct Client {
    http: HttpClient,
    base: Url,
}

impl Client {
    pub fn new() -> Result<Self> {
        Self::at(Url::parse(API)?)
    }

    fn at(base: Url) -> Result<Self> {
        Ok(Self {
            http: HttpClient::builder()
                .user_agent(concat!("Feishin-native/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            base,
        })
    }

    fn get<T: serde::de::DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
        let response = self
            .http
            .get(self.base.join(path)?)
            .query(query)
            .header("Accept", "application/vnd.github+json")
            .send()?
            .error_for_status()?;
        let mut bytes = Vec::new();
        response.take(8_000_001).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 8_000_000,
            "Release notes response is too large"
        );
        serde_json::from_slice(&bytes).context("Invalid release notes response")
    }

    pub fn history(&self) -> Result<Vec<Release>> {
        let releases: Vec<ApiRelease> = self.get("releases", &[("per_page", "30")])?;
        Ok(releases
            .into_iter()
            .filter(|release| !release.draft && tag(&release.tag_name).is_ok())
            .take(HISTORY_LIMIT)
            .map(|release| Release {
                version: release
                    .tag_name
                    .strip_prefix('v')
                    .unwrap_or(&release.tag_name)
                    .into(),
                tag: release.tag_name,
                published_at: release.published_at.chars().take(128).collect(),
                prerelease: release.prerelease,
            })
            .collect())
    }

    pub fn document(&self, version: &str, history: &[Release]) -> Result<Document> {
        let release_tag = tag(version)?;
        let page = release_page(version)?;
        if release_tag.contains("-alpha") {
            let Some(stable) = history.iter().find(|release| !release.prerelease) else {
                return Ok(Document::NoStable { page });
            };
            let stable_tag = tag(&stable.tag)?;
            let comparison: ApiComparison = self.get(
                &format!("compare/{stable_tag}...development"),
                &[("per_page", "100")],
            )?;
            let commits = comparison
                .commits
                .into_iter()
                .take(100)
                .filter(|commit| {
                    (7..=64).contains(&commit.sha.len())
                        && commit.sha.bytes().all(|byte| byte.is_ascii_hexdigit())
                        && commit.html_url == format!("{WEB}commit/{}", commit.sha)
                })
                .map(|commit| Commit {
                    sha: commit.sha,
                    message: commit.commit.message.chars().take(20_000).collect(),
                    author: commit.commit.author.name.chars().take(256).collect(),
                    date: commit.commit.author.date.chars().take(128).collect(),
                    page: commit.html_url,
                })
                .collect();
            return Ok(Document::Commits {
                stable: stable.version.clone(),
                commits,
                total: comparison.total_commits,
                page: format!("{WEB}compare/{stable_tag}...development"),
            });
        }
        let release: ApiRelease = self.get(&format!("releases/tags/{release_tag}"), &[])?;
        ensure!(
            !release.draft && tag(&release.tag_name)? == release_tag,
            "Release notes returned a different version"
        );
        Ok(Document::Markdown {
            body: release
                .body
                .unwrap_or_default()
                .chars()
                .take(100_000)
                .collect(),
            page,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{io::Write, net::TcpListener};

    #[test]
    fn release_history_stable_beta_and_alpha_use_original_github_endpoints() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let client =
            Client::at(Url::parse(&format!("http://{}/", server.local_addr().unwrap())).unwrap())
                .unwrap();
        let sha = "a".repeat(40);
        let expected_sha = sha.clone();
        let worker = std::thread::spawn(move || {
            for (path, body) in [
                (
                    "/releases?per_page=30",
                    json!([
                        {"tag_name":"v1.18.0-alpha.3","prerelease":true,"published_at":"2026-10-09T10:00:00Z"},
                        {"tag_name":"v1.17.1","prerelease":false},
                        {"tag_name":"v99.0.0","draft":true},
                        {"tag_name":"../../invalid"},
                        {"tag_name":"1.18.0-beta.1","prerelease":true}
                    ]),
                ),
                (
                    "/releases/tags/v1.17.0",
                    json!({"tag_name":"v1.17.0","body":"# Current release\n\n- **Fixed** playback"}),
                ),
                (
                    "/releases/tags/v1.18.0-beta.1",
                    json!({"tag_name":"v1.18.0-beta.1","body":null}),
                ),
                (
                    "/compare/v1.17.1...development?per_page=100",
                    json!({"total_commits":102,"commits":[
                        {"sha":sha,"html_url":format!("{WEB}commit/{sha}"),"commit":{"message":"Fix playback\n\nFull description","author":{"name":"Author","date":"2026-10-09T11:00:00Z"}}},
                        {"sha":"bad","html_url":"https://evil.invalid","commit":{"message":"Invalid","author":{"name":"Author","date":""}}}
                    ]}),
                ),
                (
                    "/releases/tags/v1.16.0",
                    json!({"tag_name":"v1.15.0","body":"Wrong release"}),
                ),
            ] {
                let (mut stream, _) = server.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                    assert!(request.len() < 16_000);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(
                    request.starts_with(&format!("GET {path} HTTP/1.1\r\n")),
                    "{request}"
                );
                assert!(!request.to_lowercase().contains("authorization:"));
                let body = serde_json::to_vec(&body).unwrap();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        let history = client.history().unwrap();
        assert!(
            fallback_page("1.18.0-alpha.3", &history)
                .unwrap()
                .ends_with("v1.17.1...v1.18.0-alpha.3")
        );
        assert!(
            fallback_page("1.18.0-alpha.3", &[])
                .unwrap()
                .ends_with("releases/tag/v1.18.0-alpha.3")
        );
        assert_eq!(
            history
                .iter()
                .map(|release| release.version.as_str())
                .collect::<Vec<_>>(),
            ["1.18.0-alpha.3", "1.17.1", "1.18.0-beta.1"]
        );
        assert!(
            matches!(client.document("1.17.0", &history).unwrap(), Document::Markdown { body, .. } if body.contains("**Fixed**"))
        );
        assert!(
            matches!(client.document("v1.18.0-beta.1", &history).unwrap(), Document::Markdown { body, .. } if body.is_empty())
        );
        let Document::Commits {
            stable,
            commits,
            total,
            page,
        } = client.document("1.18.0-alpha.3", &history).unwrap()
        else {
            panic!("Expected alpha comparison")
        };
        assert_eq!(stable, "1.17.1");
        assert_eq!(total, 102);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].sha, expected_sha);
        assert_eq!(commits[0].message, "Fix playback\n\nFull description");
        assert!(page.ends_with("v1.17.1...development"));
        assert!(matches!(
            client.document("1.18.0-alpha.3", &[]).unwrap(),
            Document::NoStable { .. }
        ));
        assert!(client.document("../bad", &history).is_err());
        assert!(client.document("1.16.0", &history).is_err());
        worker.join().unwrap();
    }
}
