//! One app per data directory, with authenticated local window activation.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, TryLockError},
    io::{ErrorKind, Read},
    net::{SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;

pub fn wait_for_exit(parent: u32, timeout: Duration) -> Result<()> {
    anyhow::ensure!(
        parent > 0 && parent != std::process::id(),
        "Invalid restart parent process"
    );
    let deadline = Instant::now() + timeout;
    while crate::updates::parent_running(parent)? {
        anyhow::ensure!(
            Instant::now() < deadline,
            "Application did not exit for restarting"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

pub fn restart_command(executable: &Path, data_dir: &Path) -> std::process::Command {
    use std::process::{Command, Stdio};
    let mut command = Command::new(executable);
    command
        .arg("--restart-after")
        .arg(std::process::id().to_string())
        .env("FEISHIN_DATA_DIR", data_dir)
        .stdin(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

#[derive(Deserialize, Serialize)]
struct Endpoint {
    address: SocketAddr,
    token: Uuid,
}

pub struct Instance {
    // Never unlink the lock file: another launcher may already have it open.
    _lock: File,
    socket: UdpSocket,
    token: Uuid,
    endpoint: PathBuf,
}

impl Instance {
    /// `None` means an existing app accepted the activation request.
    pub fn start(data_dir: &Path) -> Result<Option<Self>> {
        fs::create_dir_all(data_dir)?;
        let mut options = File::options();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(data_dir.join("instance.lock"))?;
        let endpoint = data_dir.join("instance.json");
        let sender = UdpSocket::bind("127.0.0.1:0")?;
        sender.set_read_timeout(Some(Duration::from_millis(100)))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match lock.try_lock() {
                Ok(()) => {
                    let socket = UdpSocket::bind("127.0.0.1:0")?;
                    socket.set_nonblocking(true)?;
                    let token = Uuid::new_v4();
                    let bytes = serde_json::to_vec(&Endpoint {
                        address: socket.local_addr()?,
                        token,
                    })?;
                    crate::storage::atomic_write(&endpoint, &bytes)?;
                    return Ok(Some(Self {
                        _lock: lock,
                        socket,
                        token,
                        endpoint,
                    }));
                }
                Err(TryLockError::Error(error)) => {
                    return Err(error).context("Cannot lock the application data directory");
                }
                Err(TryLockError::WouldBlock) => {}
            }
            if let Ok(target) = read_endpoint(&endpoint)
                && target.address.ip().is_loopback()
                && target.address.port() != 0
            {
                sender.send_to(target.token.as_bytes(), target.address)?;
                let mut reply = [0; 32];
                match sender.recv_from(&mut reply) {
                    Ok((length, source))
                        if source == target.address
                            && reply[..length] == *target.token.as_bytes() =>
                    {
                        return Ok(None);
                    }
                    Err(error)
                        if !matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        return Err(error.into());
                    }
                    _ => {}
                }
            } else {
                std::thread::sleep(Duration::from_millis(100));
            }
            if Instant::now() >= deadline {
                bail!(
                    "Feishin is already running but did not respond to the window activation request"
                );
            }
        }
    }

    /// Polled by the UI timer; bounded so local traffic cannot monopolize it.
    pub fn take_activation(&self) -> Result<bool> {
        let mut activation = false;
        for _ in 0..32 {
            let mut message = [0; 32];
            match self.socket.recv_from(&mut message) {
                Ok((length, sender))
                    if sender.ip().is_loopback() && message[..length] == *self.token.as_bytes() =>
                {
                    self.socket.send_to(self.token.as_bytes(), sender)?;
                    activation = true;
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(activation)
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        // The lock is still held until all fields have been dropped.
        let _ = fs::remove_file(&self.endpoint);
    }
}

fn read_endpoint(path: &Path) -> Result<Endpoint> {
    let mut bytes = Vec::new();
    File::open(path)?.take(1024).read_to_end(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        process::{Child, Command, Stdio},
        sync::mpsc,
    };

    struct ChildApp(Child);
    impl Drop for ChildApp {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn instance_child_process() {
        let Some(directory) = std::env::var_os("FEISHIN_TEST_INSTANCE_CHILD") else {
            return;
        };
        let app = Instance::start(Path::new(&directory)).unwrap().unwrap();
        println!("INSTANCE_READY");
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            if app.take_activation().unwrap() {
                println!("INSTANCE_ACTIVATED");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn second_process_activates_the_first_and_crashes_release_the_lock() {
        let directory = std::env::temp_dir().join(format!("feishin-instance-{}", Uuid::new_v4()));
        let mut child = ChildApp(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "instance::tests::instance_child_process",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("FEISHIN_TEST_INSTANCE_CHILD", &directory)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let stdout = child.0.stdout.take().unwrap();
        let (send, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if line.contains("INSTANCE_") {
                    let _ = send.send(line);
                }
            }
        });
        assert!(
            lines
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .contains("INSTANCE_READY")
        );
        let old = read_endpoint(&directory.join("instance.json")).unwrap();
        let rogue = UdpSocket::bind("127.0.0.1:0").unwrap();
        for invalid in [vec![], vec![0; 16], vec![0; 64]] {
            rogue.send_to(&invalid, old.address).unwrap();
        }
        assert!(lines.recv_timeout(Duration::from_millis(100)).is_err());
        assert!(wait_for_exit(child.0.id(), Duration::from_millis(20)).is_err());
        assert!(Instance::start(&directory).unwrap().is_none());
        assert!(
            lines
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .contains("INSTANCE_ACTIVATED")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for name in ["instance.lock", "instance.json"] {
                assert_eq!(
                    fs::metadata(directory.join(name))
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
        }
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        wait_for_exit(child.0.id(), Duration::from_secs(1)).unwrap();
        assert!(wait_for_exit(0, Duration::ZERO).is_err());
        assert!(wait_for_exit(std::process::id(), Duration::ZERO).is_err());
        assert!(directory.join("instance.json").exists());
        let recovered = Instance::start(&directory).unwrap().unwrap();
        let new = read_endpoint(&directory.join("instance.json")).unwrap();
        assert_ne!(old.token, new.token);
        rogue.send_to(old.token.as_bytes(), new.address).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert!(!recovered.take_activation().unwrap());
        rogue.send_to(new.token.as_bytes(), new.address).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert!(recovered.take_activation().unwrap());
        assert!(!recovered.take_activation().unwrap());
        let other_dir = directory.join("other-profile");
        let other = Instance::start(&other_dir).unwrap().unwrap();
        assert!(!other.take_activation().unwrap());
        drop(other);
        drop(recovered);
        assert!(!directory.join("instance.json").exists());
        assert!(directory.join("instance.lock").exists());
        let reopened = Instance::start(&directory).unwrap().unwrap();
        drop(reopened);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn restart_arguments_preserve_paths_and_profiles_without_shell_interpolation() {
        let executable = Path::new("/fixture/Feishin with spaces ' $&/feishin");
        let data = Path::new("/fixture/profile with spaces ' $&");
        let command = restart_command(executable, data);
        assert_eq!(command.get_program(), executable);
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["--restart-after".to_owned(), std::process::id().to_string()]
        );
        assert_eq!(
            command.get_envs().collect::<Vec<_>>(),
            [(
                std::ffi::OsStr::new("FEISHIN_DATA_DIR"),
                Some(data.as_os_str())
            )]
        );
    }
}
