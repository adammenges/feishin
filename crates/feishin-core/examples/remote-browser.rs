//! A disposable server for real-browser service-worker and reconnect checks.
use anyhow::{Result, bail};
use feishin_core::{model::Item, queue::Enqueue, remote::Remote, storage::State};
use serde_json::json;
use std::{
    io::{BufRead, Write},
    sync::mpsc,
    time::{Duration, Instant},
};

fn start(port: u16, state: &State) -> Result<Remote> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let remote = loop {
        match Remote::start(port, "phone", "fixture-password") {
            Ok(remote) => break remote,
            Err(error) if Instant::now() >= deadline => return Err(error),
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    remote.publish(state, false);
    Ok(remote)
}
fn report(value: serde_json::Value) -> Result<()> {
    println!("{value}");
    std::io::stdout().flush()?;
    Ok(())
}
fn main() -> Result<()> {
    let mut state = State::default();
    state.settings.volume = 50.;
    let mut remote = Some(start(0, &state)?);
    let port = remote.as_ref().unwrap().port;
    report(json!({"state": "ready", "port": port}))?;
    let (tx, commands) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    loop {
        if let Some(remote) = &remote {
            while let Ok(request) = remote.requests.try_recv() {
                if let Some(reply) = library_reply(&request.message) {
                    let _ = request.reply.send(vec![reply]);
                } else {
                    let result = queue_request(&mut state, &request.message);
                    remote.publish(&state, false);
                    let _ = request.reply.send(vec![json!({"event":"operation-ack","data":{
                        "requestId":request.message["requestId"],"error":result.err().map(|error|error.to_string())
                    }})]);
                }
            }
        }
        let line = match commands.recv_timeout(Duration::from_millis(10)) {
            Ok(line) => line?,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match line.as_str() {
            "stop" => {
                drop(remote.take());
                report(json!({"state": "stopped"}))?;
            }
            "restart" => {
                state.settings.volume = 73.;
                state.settings.use_theme_accent_color = true;
                state.settings.use_theme_primary_shade = true;
                remote = Some(start(port, &state)?);
                report(json!({"state": "ready", "port": port}))?;
            }
            "accent" => {
                state.settings.accent = "#ff0000".into();
                state.settings.primary_shade = 3;
                state.settings.use_theme_accent_color = false;
                state.settings.use_theme_primary_shade = false;
                remote.as_ref().unwrap().publish(&state, false);
                report(json!({"state": "updated"}))?;
            }
            "credentials" | "unprotected" | "protected" => {
                let (username, password) = match line.as_str() {
                    "unprotected" => ("", ""),
                    "protected" => ("phone", "fixture-password"),
                    _ => ("phone", "changed-password"),
                };
                remote
                    .as_ref()
                    .unwrap()
                    .update_credentials(username, password)?;
                report(json!({"state": "updated", "port": port}))?;
            }
            "large-queue" => {
                state.queue.add(
                    (0..38_000)
                        .map(|index| Item {
                            id: "duplicate-track".into(),
                            name: format!("Queue track {index}"),
                            artist: "Fixture artist".into(),
                            duration: 180.,
                            ..Default::default()
                        })
                        .collect(),
                    Enqueue::Now,
                );
                remote.as_ref().unwrap().publish(&state, false);
                report(json!({"state": "updated"}))?;
            }
            "quit" => break,
            _ => bail!("Unknown fixture command"),
        }
    }
    Ok(())
}

fn queue_request(state: &mut State, value: &serde_json::Value) -> Result<()> {
    if value["event"] == "reorder-queue" {
        return feishin_core::remote::reorder_queue(&mut state.queue, value);
    }
    let key = value["uniqueId"]
        .as_str()
        .and_then(|id| uuid::Uuid::parse_str(id).ok());
    let key = key
        .filter(|key| state.queue.order.contains(key))
        .ok_or_else(|| anyhow::anyhow!("Queue entry no longer exists"))?;
    match value["event"].as_str() {
        Some("remove-from-queue") => {
            state.queue.remove(key);
        }
        Some("queue-jump") => {
            state.queue.current = Some(key);
            state.queue.position = 0.;
        }
        event => bail!("Unhandled browser fixture event: {event:?}"),
    }
    Ok(())
}

fn library_reply(value: &serde_json::Value) -> Option<serde_json::Value> {
    let kind = value["event"].as_str()?.strip_suffix("-request")?;
    if !matches!(kind, "tracks" | "albums" | "playlists" | "radio") {
        return None;
    }
    let term = value["searchTerm"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    let items = (0..320).map(|index| json!({
        "id":format!("library-{kind}-{index}"),"name":format!("Library {kind} {index}"),"artistName":"Fixture artist","duration":180
    })).filter(|item| item["name"].as_str().unwrap().to_lowercase().contains(&term)).collect::<Vec<_>>();
    let offset = value["startIndex"].as_u64().unwrap_or(0) as usize;
    let limit = value["limit"].as_u64().unwrap_or(50).min(50) as usize;
    let end = offset.saturating_add(limit).min(items.len());
    Some(json!({"event":format!("{kind}-response"),"data":{
        "requestId":value["requestId"],"items":&items[offset.min(items.len())..end],"hasMore":end<items.len()
    }}))
}
