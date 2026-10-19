//! Line-preserving lyric translation. Secrets stay in the OS credential store.
use anyhow::{Context, Result, ensure};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Read, time::Duration};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub provider: String,
    pub language: String,
    pub automatic: bool,
    pub azure_region: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            provider: String::new(),
            language: "en".into(),
            automatic: false,
            azure_region: String::new(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(
                self.provider.as_str(),
                "" | "Microsoft Azure" | "Google Cloud"
            ),
            "Choose an Azure or Google translation provider"
        );
        ensure!(
            !self.language.is_empty()
                && self.language.len() <= 32
                && self
                    .language
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "Invalid translation language code"
        );
        ensure!(
            self.azure_region.len() <= 128
                && self
                    .azure_region
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "Invalid Azure region"
        );
        Ok(())
    }
}
pub fn translate(config: &Config, key: &str, lines: &[String]) -> Result<Vec<String>> {
    let endpoint = match config.provider.as_str() {
        "Microsoft Azure" => "https://api.cognitive.microsofttranslator.com/translate",
        "Google Cloud" => "https://translation.googleapis.com/language/translate/v2",
        _ => anyhow::bail!("Choose a translation provider in Settings"),
    };
    translate_at(config, key, lines, endpoint)
}
fn translate_at(
    config: &Config,
    key: &str,
    lines: &[String],
    endpoint: &str,
) -> Result<Vec<String>> {
    config.validate()?;
    ensure!(
        !key.is_empty() && key.len() <= 4096 && !key.chars().any(char::is_control),
        "Enter a valid translation API key"
    );
    ensure!(
        lines.len() <= 10_000
            && lines.iter().map(String::len).sum::<usize>() <= 1_000_000
            && lines.iter().all(|line| line.chars().count() <= 5000),
        "Lyrics exceed the translation service limits"
    );
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut translated = vec![String::new(); lines.len()];
    let mut start = 0;
    while start < lines.len() {
        let mut end = start;
        let mut count = 0;
        while end < lines.len() && end - start < 100 && count + lines[end].chars().count() <= 5000 {
            count += lines[end].chars().count();
            end += 1;
        }
        let selected: Vec<_> = (start..end)
            .filter(|&index| !lines[index].trim().is_empty())
            .collect();
        if !selected.is_empty() {
            let mut request = client.post(endpoint);
            if config.provider == "Microsoft Azure" {
                request = request
                    .query(&[("api-version", "3.0"), ("to", &config.language)])
                    .header("Ocp-Apim-Subscription-Key", key)
                    .json(
                        &selected
                            .iter()
                            .map(|&index| json!({"Text":lines[index]}))
                            .collect::<Vec<_>>(),
                    );
                if !config.azure_region.is_empty() {
                    request = request.header("Ocp-Apim-Subscription-Region", &config.azure_region);
                }
            } else {
                request = request.query(&[("key", key)]).json(&json!({"format":"text","target":config.language,"q":selected.iter().map(|&index| &lines[index]).collect::<Vec<_>>()}));
            }
            let response = request
                .send()
                .map_err(|error| error.without_url())?
                .error_for_status()
                .map_err(|error| error.without_url())?;
            let mut bytes = vec![];
            response.take(2_000_001).read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 2_000_000,
                "Translation response is too large"
            );
            let value: Value = serde_json::from_slice(&bytes)
                .context("Translation service returned invalid JSON")?;
            let results = if config.provider == "Microsoft Azure" {
                value.as_array()
            } else {
                value["data"]["translations"].as_array()
            }
            .context("Translation service returned no translations")?;
            ensure!(
                results.len() == selected.len(),
                "Translation service returned the wrong number of lines"
            );
            for (&index, result) in selected.iter().zip(results) {
                translated[index] = if config.provider == "Microsoft Azure" {
                    result["translations"][0]["text"].as_str()
                } else {
                    result["translatedText"].as_str()
                }
                .context("Translation service returned an invalid line")?
                .to_owned();
            }
        }
        start = end;
    }
    Ok(translated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    #[test]
    fn azure_and_google_preserve_lines_batches_region_and_private_credentials() {
        for provider in ["Microsoft Azure", "Google Cloud"] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}/translate", listener.local_addr().unwrap());
            let worker = thread::spawn(move || {
                for _ in 0..2 {
                    let (mut socket, _) = listener.accept().unwrap();
                    socket
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut bytes = vec![];
                    let mut buffer = [0; 8192];
                    let (header_len, content_len) = loop {
                        let count = socket.read(&mut buffer).unwrap();
                        assert!(count > 0);
                        bytes.extend_from_slice(&buffer[..count]);
                        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&bytes[..index]).to_lowercase();
                            let length: usize = headers
                                .lines()
                                .find_map(|line| line.strip_prefix("content-length: "))
                                .unwrap()
                                .parse()
                                .unwrap();
                            break (index + 4, length);
                        }
                    };
                    while bytes.len() < header_len + content_len {
                        let count = socket.read(&mut buffer).unwrap();
                        assert!(count > 0);
                        bytes.extend_from_slice(&buffer[..count]);
                    }
                    let headers = String::from_utf8_lossy(&bytes[..header_len]).to_lowercase();
                    let body: Value = serde_json::from_slice(&bytes[header_len..]).unwrap();
                    let response = if provider == "Microsoft Azure" {
                        assert!(headers.contains("ocp-apim-subscription-key: fixture-key"));
                        assert!(headers.contains("ocp-apim-subscription-region: eastus"));
                        assert!(headers.contains("to=ja"));
                        Value::Array(body.as_array().unwrap().iter().map(|line| json!({"translations":[{"text":format!("訳:{}",line["Text"].as_str().unwrap())}]})).collect())
                    } else {
                        assert!(headers.contains("key=fixture-key"));
                        assert_eq!(body["target"],"ja");
                        json!({"data":{"translations":body["q"].as_array().unwrap().iter().map(|line| json!({"translatedText":format!("訳:{}",line.as_str().unwrap())})).collect::<Vec<_>>()}})
                    }.to_string();
                    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).unwrap();
                }
            });
            let config = Config {
                provider: provider.into(),
                language: "ja".into(),
                automatic: false,
                azure_region: "eastus".into(),
            };
            let lines = vec!["第一行".into(), "".into(), "x".repeat(4997), "Last".into()];
            let translated = translate_at(&config, "fixture-key", &lines, &endpoint).unwrap();
            assert_eq!(translated[0], "訳:第一行");
            assert_eq!(translated[1], "");
            assert_eq!(translated[3], "訳:Last");
            assert!(
                !serde_json::to_string(&config)
                    .unwrap()
                    .contains("fixture-key")
            );
            worker.join().unwrap();
            assert!(translate_at(&config, "bad\nkey", &[], &endpoint).is_err());
        }
    }
}
