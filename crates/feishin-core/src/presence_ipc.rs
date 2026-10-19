//! Bounded Discord IPC transport; a stalled peer cannot block privacy updates forever.
use discord_rich_presence::{DiscordIpc, error::Error};
use interprocess::{
    ConnectWaitMode,
    local_socket::{ConnectOptions, Stream, prelude::*},
};
use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    thread,
    time::{Duration, Instant},
};

pub struct Client {
    pub client_id: String,
    socket: Option<Stream>,
}
impl Client {
    pub fn new(id: &str) -> Self {
        Self {
            client_id: id.into(),
            socket: None,
        }
    }
    pub fn reply(&mut self, nonce: &str) -> Result<Value, Error> {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            let (opcode, value) = self.recv()?;
            if opcode == 3 {
                self.send(value, 4)?;
            } else if opcode == 1 && value["nonce"].as_str() == Some(nonce) {
                return Ok(value);
            } else if opcode == 2 {
                return Err(Error::IPCConnectionFailed);
            }
        }
        Err(Error::ReadError(io::Error::new(
            io::ErrorKind::TimedOut,
            "Discord reply timed out",
        )))
    }
    fn connect_name(name: interprocess::local_socket::Name<'_>) -> io::Result<Stream> {
        ConnectOptions::new()
            .name(name)
            .wait_mode(ConnectWaitMode::Timeout(Duration::from_secs(1)))
            .nonblocking_stream(true)
            .connect_sync()
    }
}
fn retry(mut operation: impl FnMut() -> io::Result<usize>, deadline: Instant) -> io::Result<usize> {
    loop {
        match operation() {
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Discord IPC timed out",
                    ));
                }
                thread::sleep(Duration::from_millis(5));
            }
            result => return result,
        }
    }
}
impl DiscordIpc for Client {
    fn get_client_id(&self) -> &str {
        &self.client_id
    }
    fn connect_ipc(&mut self) -> Result<(), Error> {
        #[cfg(windows)]
        {
            use interprocess::local_socket::GenericNamespaced;
            for i in 0..10 {
                let name = format!("discord-ipc-{i}");
                if let Ok(name) = name.to_ns_name::<GenericNamespaced>()
                    && let Ok(socket) = Self::connect_name(name)
                {
                    self.socket = Some(socket);
                    return Ok(());
                }
            }
        }
        #[cfg(unix)]
        {
            use interprocess::local_socket::GenericFilePath;
            let mut roots = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
                .iter()
                .filter_map(|key| std::env::var_os(key).map(std::path::PathBuf::from))
                .collect::<Vec<_>>();
            roots.push(std::env::temp_dir());
            if std::env::var_os("SNAP").is_some()
                && let Some(root) = roots
                    .first()
                    .and_then(|root| root.parent())
                    .map(std::path::Path::to_owned)
            {
                roots.insert(0, root);
            }
            for root in roots {
                for subpath in [
                    "",
                    "app/com.discordapp.Discord",
                    "app/dev.vencord.Vesktop",
                    ".flatpak/com.discordapp.Discord/xdg-run",
                    ".flatpak/dev.vencord.Vesktop/xdg-run",
                    "snap.discord-canary",
                    "snap.discord",
                ] {
                    for i in 0..10 {
                        let path = root.join(subpath).join(format!("discord-ipc-{i}"));
                        if !path.exists() {
                            continue;
                        }
                        if let Ok(name) = path.to_fs_name::<GenericFilePath>()
                            && let Ok(socket) = Self::connect_name(name)
                        {
                            self.socket = Some(socket);
                            return Ok(());
                        }
                    }
                }
            }
        }
        Err(Error::IPCNotFound)
    }
    fn send_handshake(&mut self) -> Result<(), Error> {
        self.send(json!({"v":1,"client_id":self.client_id}), 0)?;
        let (opcode, value) = self.recv()?;
        if opcode != 1 || value["evt"] != "READY" {
            return Err(Error::IPCConnectionFailed);
        }
        Ok(())
    }
    fn write(&mut self, mut data: &[u8]) -> Result<(), Error> {
        let socket = self.socket.as_mut().ok_or(Error::NotConnected)?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while !data.is_empty() {
            let count = retry(|| socket.write(data), deadline).map_err(Error::WriteError)?;
            if count == 0 {
                return Err(Error::WriteError(io::ErrorKind::WriteZero.into()));
            }
            data = &data[count..];
        }
        Ok(())
    }
    fn read(&mut self, mut data: &mut [u8]) -> Result<(), Error> {
        let socket = self.socket.as_mut().ok_or(Error::NotConnected)?;
        let deadline = Instant::now() + Duration::from_secs(1);
        while !data.is_empty() {
            let count = retry(|| socket.read(data), deadline).map_err(Error::ReadError)?;
            if count == 0 {
                return Err(Error::ReadError(io::ErrorKind::UnexpectedEof.into()));
            }
            data = &mut data[count..];
        }
        Ok(())
    }
    fn recv(&mut self) -> Result<(u32, Value), Error> {
        let mut header = [0; 8];
        self.read(&mut header)?;
        let opcode = u32::from_le_bytes(header[..4].try_into().unwrap());
        let length = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
        if length > 1_048_576 {
            return Err(Error::DecodeHeader);
        }
        let mut data = vec![0; length];
        self.read(&mut data)?;
        let value = serde_json::from_slice(&data).map_err(|_| Error::JsonParseResponse)?;
        Ok((opcode, value))
    }
    fn close(&mut self) -> Result<(), Error> {
        let _ = self.send(json!({}), 2);
        self.socket = None;
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use interprocess::local_socket::{GenericFilePath, ListenerOptions};
    #[test]
    fn stalled_and_oversized_discord_replies_are_bounded_and_activity_can_be_cleared() {
        let path = std::env::temp_dir().join(format!("fd-{}.sock", uuid::Uuid::new_v4().simple()));
        let listener = ListenerOptions::new()
            .name(path.clone().to_fs_name::<GenericFilePath>().unwrap())
            .create_sync()
            .unwrap();
        let worker = thread::spawn(move || {
            let mut stream = listener.accept().unwrap();
            let read = |stream: &mut Stream| {
                let mut header = [0; 8];
                stream.read_exact(&mut header).unwrap();
                let count = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
                let mut bytes = vec![0; count];
                stream.read_exact(&mut bytes).unwrap();
                serde_json::from_slice::<Value>(&bytes).unwrap()
            };
            let handshake = read(&mut stream);
            assert_eq!(handshake["client_id"], "test");
            let ready = br#"{"evt":"READY"}"#;
            stream.write_all(&1u32.to_le_bytes()).unwrap();
            stream
                .write_all(&(ready.len() as u32).to_le_bytes())
                .unwrap();
            stream.write_all(ready).unwrap();
            let activity = read(&mut stream);
            assert_eq!(activity["args"]["activity"]["details"], "Song");
            let clear = read(&mut stream);
            assert!(clear["args"]["activity"].is_null());
            stream.write_all(&1u32.to_le_bytes()).unwrap();
            stream.write_all(&2_000_000u32.to_le_bytes()).unwrap();
            let _ = read(&mut stream);
        });
        let socket = Client::connect_name(path.to_fs_name::<GenericFilePath>().unwrap()).unwrap();
        let mut client = Client {
            client_id: "test".into(),
            socket: Some(socket),
        };
        client.send_handshake().unwrap();
        client
            .send(
                json!({"cmd":"SET_ACTIVITY","args":{"activity":{"details":"Song"}}}),
                1,
            )
            .unwrap();
        let before = Instant::now();
        assert!(matches!(client.recv(), Err(Error::ReadError(_))));
        assert!(before.elapsed() < Duration::from_secs(2));
        client.clear_activity().unwrap();
        assert!(matches!(client.recv(), Err(Error::DecodeHeader)));
        client.close().unwrap();
        worker.join().unwrap();
    }
}
