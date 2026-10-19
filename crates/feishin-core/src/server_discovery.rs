use anyhow::Result;
use serde::Deserialize;
use std::{
    collections::HashSet,
    io::ErrorKind,
    net::{SocketAddr, UdpSocket},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    pub url: String,
}

fn response(bytes: &[u8]) -> Option<Server> {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Response {
        address: String,
        name: String,
    }
    let value: Response = serde_json::from_slice(bytes).ok()?;
    if value.address.len() > 2048
        || value.name.len() > 1024
        || value.name.trim().is_empty()
        || value.name.chars().any(char::is_control)
    {
        return None;
    }
    Some(Server {
        name: value.name,
        url: crate::api::normalize_url(&value.address).ok()?,
    })
}

pub fn jellyfin(cancelled: &AtomicBool, found: impl FnMut(Server) -> bool) -> Result<()> {
    discover(
        &[
            "127.255.255.255:7359".parse()?,
            "255.255.255.255:7359".parse()?,
        ],
        Duration::from_secs(3),
        cancelled,
        found,
    )
}

fn discover(
    targets: &[SocketAddr],
    duration: Duration,
    cancelled: &AtomicBool,
    mut found: impl FnMut(Server) -> bool,
) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Ok(());
    }
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(50)))?;
    let mut sent = false;
    let mut last_error = None;
    for target in targets {
        match socket.send_to(b"who is JellyfinServer?", target) {
            Ok(_) => sent = true,
            Err(error) => last_error = Some(error),
        }
    }
    if !sent && let Some(error) = last_error {
        return Err(error.into());
    }
    let deadline = Instant::now() + duration;
    let mut seen = HashSet::new();
    while Instant::now() < deadline && !cancelled.load(Ordering::Relaxed) && seen.len() < 64 {
        let mut bytes = [0; 8192];
        match socket.recv_from(&mut bytes) {
            Ok((length, _)) if length < bytes.len() => {
                if let Some(server) = response(&bytes[..length])
                    && seen.insert(server.url.clone())
                    && !found(server)
                {
                    break;
                }
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn local_jellyfin_discovery_streams_unique_valid_servers_and_cancels() {
        let fixture = UdpSocket::bind("127.0.0.1:0").unwrap();
        fixture
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let target = fixture.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let mut request = [0; 128];
            let (length, sender) = fixture.recv_from(&mut request).unwrap();
            assert_eq!(&request[..length], b"who is JellyfinServer?");
            for bytes in [b"invalid JSON".to_vec(), serde_json::to_vec(&json!({"Address":"javascript:bad","Name":"Invalid"})).unwrap(),
                serde_json::to_vec(&json!({"Address":"http://127.0.0.1:8096/","Name":"Living room","Id":"fixture"})).unwrap(),
                serde_json::to_vec(&json!({"Address":"http://127.0.0.1:8096","Name":"Duplicate"})).unwrap(),
                serde_json::to_vec(&json!({"Address":"https://localhost:8920/music","Name":"音乐"})).unwrap()] {
                fixture.send_to(&bytes, sender).unwrap();
            }
        });
        let mut servers = Vec::new();
        let cancelled = AtomicBool::new(false);
        discover(
            &[target],
            Duration::from_millis(150),
            &cancelled,
            |server| {
                servers.push(server);
                true
            },
        )
        .unwrap();
        worker.join().unwrap();
        assert_eq!(
            servers,
            [
                Server {
                    name: "Living room".into(),
                    url: "http://127.0.0.1:8096".into()
                },
                Server {
                    name: "音乐".into(),
                    url: "https://localhost:8920/music".into()
                }
            ]
        );
        cancelled.store(true, Ordering::Relaxed);
        let start = Instant::now();
        discover(&[], Duration::from_secs(3), &cancelled, |_| {
            panic!("Cancelled discovery produced a result")
        })
        .unwrap();
        assert!(start.elapsed() < Duration::from_millis(100));
        for value in [
            json!({"Address":"file:///etc/passwd","Name":"Bad"}),
            json!({"Address":"http://user:secret@localhost","Name":"Bad"}),
            json!({"Address":"http://localhost","Name":"\nBad"}),
            json!({"Address":"http://localhost","Name":""}),
        ] {
            assert!(response(&serde_json::to_vec(&value).unwrap()).is_none());
        }
    }
}
