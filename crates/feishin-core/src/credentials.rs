//! Linux password-store selection. Other platforms retain the native keyring.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Backend {
    #[default]
    #[serde(rename = "gnome_libsecret", alias = "gnome-libsecret")]
    SecretService,
    #[serde(rename = "kwallet")]
    KWallet4,
    #[serde(rename = "kwallet5")]
    KWallet5,
    #[serde(rename = "kwallet6")]
    KWallet6,
}
impl Backend {
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::SecretService, "libsecret"),
        (Self::KWallet4, "KDE 4 (kwallet4)"),
        (Self::KWallet5, "KDE 5 (kwallet5)"),
        (Self::KWallet6, "KDE 6 (kwallet6)"),
    ];
    pub fn label(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(backend, _)| *backend == self)
            .unwrap()
            .1
    }
}

/// Called once before loading credentials. Changing the preference requires a
/// restart so existing entries and in-flight keychain transactions stay bound
/// to their original store.
pub fn configure(backend: Backend) {
    #[cfg(target_os = "linux")]
    if backend != Backend::SecretService {
        keyring::set_default_credential_builder(Box::new(linux::Builder(backend)));
    }
    #[cfg(target_os = "linux")]
    ACTIVE.store(
        match backend {
            Backend::SecretService => 0,
            Backend::KWallet4 => 4,
            Backend::KWallet5 => 5,
            Backend::KWallet6 => 6,
        },
        std::sync::atomic::Ordering::Relaxed,
    );
    #[cfg(not(target_os = "linux"))]
    let _ = backend;
}

#[cfg(target_os = "linux")]
static ACTIVE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn active_backend() -> Backend {
    #[cfg(target_os = "linux")]
    return match ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
        4 => Backend::KWallet4,
        5 => Backend::KWallet5,
        6 => Backend::KWallet6,
        _ => Backend::SecretService,
    };
    #[cfg(not(target_os = "linux"))]
    Backend::SecretService
}

/// Import into the selected destination even before its restart takes effect.
pub fn entry_for_backend(
    backend: Backend,
    service: &str,
    user: &str,
) -> keyring::Result<keyring::Entry> {
    #[cfg(target_os = "linux")]
    {
        use keyring::credential::CredentialBuilderApi;
        let credential = if backend == Backend::SecretService {
            keyring::secret_service::default_credential_builder().build(None, service, user)?
        } else {
            linux::Builder(backend).build(None, service, user)?
        };
        Ok(keyring::Entry::new_with_credential(credential))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = backend;
        keyring::Entry::new(service, user)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::Backend;
    use dbus::blocking::{Connection, Proxy};
    use keyring::{
        Error, Result,
        credential::{Credential, CredentialApi, CredentialBuilderApi},
    };
    use std::{any::Any, time::Duration};

    const APP: &str = "Feishin Native";
    const INTERFACE: &str = "org.kde.KWallet";
    const TIMEOUT: Duration = Duration::from_secs(15);

    fn endpoint(backend: Backend) -> (&'static str, &'static str) {
        match backend {
            Backend::KWallet4 => ("org.kde.kwalletd", "/modules/kwalletd"),
            Backend::KWallet5 => ("org.kde.kwalletd5", "/modules/kwalletd5"),
            Backend::KWallet6 => ("org.kde.kwalletd6", "/modules/kwalletd6"),
            Backend::SecretService => unreachable!("libsecret uses the default keyring builder"),
        }
    }
    fn bus_error(error: dbus::Error) -> Error {
        // Remote D-Bus errors may include arguments. Keep credentials out of the
        // user-facing/logged error while retaining the error category.
        Error::NoStorageAccess(Box::new(std::io::Error::other(format!(
            "KWallet request failed ({})",
            error.name().unwrap_or("D-Bus error")
        ))))
    }
    fn failure(message: &str) -> Error {
        Error::NoStorageAccess(Box::new(std::io::Error::other(message.to_owned())))
    }

    pub struct Builder(pub Backend);
    impl CredentialBuilderApi for Builder {
        fn build(
            &self,
            target: Option<&str>,
            service: &str,
            user: &str,
        ) -> Result<Box<Credential>> {
            for (name, value) in [("service", service), ("user", user)] {
                if value.len() > 4096 {
                    return Err(Error::TooLong(name.into(), 4096));
                }
                if value.contains('\0') {
                    return Err(Error::Invalid(
                        name.into(),
                        "Contains a null character".into(),
                    ));
                }
            }
            Ok(Box::new(Entry {
                backend: self.0,
                folder: service.to_owned(),
                key: user.to_owned(),
                wallet: target.map(str::to_owned),
            }))
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }
    #[derive(Debug)]
    struct Entry {
        backend: Backend,
        folder: String,
        key: String,
        wallet: Option<String>,
    }
    enum Operation<'a> {
        Read,
        Write(&'a str),
        Delete,
    }
    struct Wallet<'a> {
        proxy: Proxy<'a, &'a Connection>,
        handle: i32,
    }
    impl Drop for Wallet<'_> {
        fn drop(&mut self) {
            let _: std::result::Result<(i32,), _> =
                self.proxy
                    .method_call(INTERFACE, "close", (self.handle, false, APP));
        }
    }
    impl Entry {
        fn run(&self, operation: Operation<'_>) -> Result<Vec<u8>> {
            let connection = Connection::new_session().map_err(bus_error)?;
            self.with_connection(&connection, operation)
        }
        fn with_connection(
            &self,
            connection: &Connection,
            operation: Operation<'_>,
        ) -> Result<Vec<u8>> {
            let (service, path) = endpoint(self.backend);
            let mut proxy = connection.with_proxy(service, path, TIMEOUT);
            let wallet = if let Some(wallet) = &self.wallet {
                wallet.clone()
            } else {
                let (wallet,): (String,) = proxy
                    .method_call(INTERFACE, "networkWallet", ())
                    .map_err(bus_error)?;
                wallet
            };
            if wallet.is_empty() {
                return Err(failure("KWallet has no network wallet"));
            }
            // Opening may prompt the user to unlock their wallet.
            proxy.timeout = Duration::from_secs(120);
            let (handle,): (i32,) = proxy
                .method_call(INTERFACE, "open", (&wallet, 0i64, APP))
                .map_err(bus_error)?;
            if handle < 0 {
                return Err(failure("KWallet access was denied"));
            }
            proxy.timeout = TIMEOUT;
            let wallet = Wallet { proxy, handle };
            let (exists,): (bool,) = wallet
                .proxy
                .method_call(INTERFACE, "hasFolder", (handle, &self.folder, APP))
                .map_err(bus_error)?;
            if !exists {
                if !matches!(operation, Operation::Write(_)) {
                    return Err(Error::NoEntry);
                }
                let (created,): (bool,) = wallet
                    .proxy
                    .method_call(INTERFACE, "createFolder", (handle, &self.folder, APP))
                    .map_err(bus_error)?;
                if !created {
                    return Err(failure("KWallet could not create the credential folder"));
                }
            }
            if !matches!(operation, Operation::Write(_)) {
                let (exists,): (bool,) = wallet
                    .proxy
                    .method_call(
                        INTERFACE,
                        "hasEntry",
                        (handle, &self.folder, &self.key, APP),
                    )
                    .map_err(bus_error)?;
                if !exists {
                    return Err(Error::NoEntry);
                }
            }
            match operation {
                Operation::Read => {
                    let (password,): (String,) = wallet
                        .proxy
                        .method_call(
                            INTERFACE,
                            "readPassword",
                            (handle, &self.folder, &self.key, APP),
                        )
                        .map_err(bus_error)?;
                    Ok(password.into_bytes())
                }
                Operation::Write(password) => {
                    let (result,): (i32,) = wallet
                        .proxy
                        .method_call(
                            INTERFACE,
                            "writePassword",
                            (handle, &self.folder, &self.key, password, APP),
                        )
                        .map_err(bus_error)?;
                    if result != 0 {
                        return Err(failure("KWallet could not save the credential"));
                    }
                    Ok(Vec::new())
                }
                Operation::Delete => {
                    let (result,): (i32,) = wallet
                        .proxy
                        .method_call(
                            INTERFACE,
                            "removeEntry",
                            (handle, &self.folder, &self.key, APP),
                        )
                        .map_err(bus_error)?;
                    if result != 0 {
                        return Err(failure("KWallet could not remove the credential"));
                    }
                    Ok(Vec::new())
                }
            }
        }
    }
    impl CredentialApi for Entry {
        fn set_secret(&self, secret: &[u8]) -> Result<()> {
            let password =
                std::str::from_utf8(secret).map_err(|_| Error::BadEncoding(secret.to_vec()))?;
            self.run(Operation::Write(password)).map(|_| ())
        }
        fn get_secret(&self) -> Result<Vec<u8>> {
            self.run(Operation::Read)
        }
        fn delete_credential(&self) -> Result<()> {
            self.run(Operation::Delete).map(|_| ())
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn debug_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("KWalletCredential")
                .field("backend", &self.backend)
                .finish_non_exhaustive()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use dbus::{
            channel::{MatchingReceiver, Sender},
            message::MatchRule,
        };
        use std::{
            collections::{HashMap, HashSet},
            io::{BufRead, BufReader},
            process::{Child, Command, Stdio},
            sync::{
                Arc, Mutex,
                atomic::{AtomicBool, Ordering},
            },
            thread,
        };

        struct Fixture {
            daemon: Child,
            stop: Arc<AtomicBool>,
            server: Option<thread::JoinHandle<()>>,
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                self.stop.store(true, Ordering::Relaxed);
                if let Some(server) = self.server.take() {
                    server.join().unwrap();
                }
                let _ = self.daemon.kill();
                let _ = self.daemon.wait();
            }
        }

        #[test]
        fn kwallet_protocol_uses_each_original_endpoint_and_preserves_missing_empty_failed_entries()
        {
            let mut daemon = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("Linux credential tests require dbus-daemon");
            let mut address = String::new();
            BufReader::new(daemon.stdout.take().unwrap())
                .read_line(&mut address)
                .unwrap();
            let address = address.trim();
            let service = Connection::new_address(address).unwrap();
            for backend in [Backend::KWallet4, Backend::KWallet5, Backend::KWallet6] {
                service
                    .request_name(endpoint(backend).0, false, false, true)
                    .unwrap();
            }
            let calls = Arc::new(Mutex::new(Vec::new()));
            let captured = calls.clone();
            let mut folders = HashSet::<String>::new();
            let mut entries = HashMap::<(String, String), String>::new();
            service.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |message, connection| {
                    assert_eq!(message.interface().unwrap().to_string(), INTERFACE);
                    let destination = message.destination().unwrap().to_string();
                    let path = message.path().unwrap().to_string();
                    let expected_path = match destination.as_str() {
                        "org.kde.kwalletd" => "/modules/kwalletd",
                        "org.kde.kwalletd5" => "/modules/kwalletd5",
                        "org.kde.kwalletd6" => "/modules/kwalletd6",
                        other => panic!("Unexpected credential destination: {other}"),
                    };
                    assert_eq!(path, expected_path);
                    let method = message.member().unwrap().to_string();
                    captured.lock().unwrap().push((destination, method.clone()));
                    let reply = match method.as_str() {
                        "networkWallet" => message.method_return().append1("fixture-wallet"),
                        "open" => {
                            let (wallet, window, app): (String, i64, String) =
                                message.read3().unwrap();
                            assert_eq!(window, 0);
                            assert_eq!(app, APP);
                            message.method_return().append1(if wallet == "denied" {
                                -1i32
                            } else {
                                42i32
                            })
                        }
                        "close" => {
                            let (handle, force, app): (i32, bool, String) =
                                message.read3().unwrap();
                            assert_eq!((handle, force, app.as_str()), (42, false, APP));
                            message.method_return().append1(0i32)
                        }
                        "hasFolder" | "createFolder" => {
                            let (handle, folder, app): (i32, String, String) =
                                message.read3().unwrap();
                            assert_eq!(handle, 42);
                            assert_eq!(app, APP);
                            let exists = if method == "createFolder" {
                                folders.insert(folder);
                                true
                            } else {
                                folders.contains(&folder)
                            };
                            message.method_return().append1(exists)
                        }
                        "hasEntry" | "readPassword" | "removeEntry" => {
                            let (handle, folder, key, app): (i32, String, String, String) =
                                message.read4().unwrap();
                            assert_eq!(handle, 42);
                            assert_eq!(app, APP);
                            let key = (folder, key);
                            match method.as_str() {
                                "hasEntry" => {
                                    message.method_return().append1(entries.contains_key(&key))
                                }
                                "removeEntry" => {
                                    entries.remove(&key);
                                    message.method_return().append1(0i32)
                                }
                                _ => message
                                    .method_return()
                                    .append1(entries.get(&key).unwrap().clone()),
                            }
                        }
                        "writePassword" => {
                            let (handle, folder, key, password, app): (
                                i32,
                                String,
                                String,
                                String,
                                String,
                            ) = message.read5().unwrap();
                            assert_eq!(handle, 42);
                            assert_eq!(app, APP);
                            if password == "remote-failure-credential" {
                                message.error(
                                    &"org.kde.KWallet.Error".into(),
                                    &std::ffi::CString::new(password).unwrap(),
                                )
                            } else if password == "rejected" {
                                message.method_return().append1(1i32)
                            } else {
                                entries.insert((folder, key), password);
                                message.method_return().append1(0i32)
                            }
                        }
                        _ => panic!("Unexpected KWallet method: {method}"),
                    };
                    connection.send(reply).unwrap();
                    true
                }),
            );
            let stop = Arc::new(AtomicBool::new(false));
            let running = stop.clone();
            let server = thread::spawn(move || {
                while !running.load(Ordering::Relaxed) {
                    service.process(Duration::from_millis(20)).unwrap();
                }
            });
            let _fixture = Fixture {
                daemon,
                stop,
                server: Some(server),
            };
            let connection = Connection::new_address(address).unwrap();
            for backend in [Backend::KWallet4, Backend::KWallet5, Backend::KWallet6] {
                let entry = Entry {
                    backend,
                    folder: format!("service-{backend:?}"),
                    key: "account".into(),
                    wallet: None,
                };
                assert!(matches!(
                    entry.with_connection(&connection, Operation::Read),
                    Err(Error::NoEntry)
                ));
                entry
                    .with_connection(&connection, Operation::Write("密碼 🎶"))
                    .unwrap();
                assert_eq!(
                    entry.with_connection(&connection, Operation::Read).unwrap(),
                    "密碼 🎶".as_bytes()
                );
                assert!(
                    entry
                        .with_connection(&connection, Operation::Write("rejected"))
                        .is_err()
                );
                assert_eq!(
                    entry.with_connection(&connection, Operation::Read).unwrap(),
                    "密碼 🎶".as_bytes()
                );
                let error = entry
                    .with_connection(&connection, Operation::Write("remote-failure-credential"))
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("org.kde.KWallet.Error"));
                assert!(!error.contains("remote-failure-credential"));
                entry
                    .with_connection(&connection, Operation::Write(""))
                    .unwrap();
                assert_eq!(
                    entry.with_connection(&connection, Operation::Read).unwrap(),
                    b""
                );
                entry
                    .with_connection(&connection, Operation::Delete)
                    .unwrap();
                assert!(matches!(
                    entry.with_connection(&connection, Operation::Read),
                    Err(Error::NoEntry)
                ));
                assert!(matches!(
                    entry.with_connection(&connection, Operation::Delete),
                    Err(Error::NoEntry)
                ));
                let denied = Entry {
                    backend,
                    folder: entry.folder.clone(),
                    key: entry.key.clone(),
                    wallet: Some("denied".into()),
                };
                assert!(matches!(
                    denied.with_connection(&connection, Operation::Read),
                    Err(Error::NoStorageAccess(_))
                ));
                let before = calls.lock().unwrap().len();
                assert!(matches!(
                    entry.set_secret(&[255]),
                    Err(Error::BadEncoding(_))
                ));
                assert_eq!(calls.lock().unwrap().len(), before);
                let descriptor = Builder(backend)
                    .build(None, &entry.folder, &entry.key)
                    .unwrap();
                assert!(descriptor.as_any().is::<Entry>());
            }
            let calls = calls.lock().unwrap();
            for backend in [Backend::KWallet4, Backend::KWallet5, Backend::KWallet6] {
                let destination = endpoint(backend).0;
                assert_eq!(
                    calls
                        .iter()
                        .filter(|(name, method)| name == destination && method == "open")
                        .count(),
                    calls
                        .iter()
                        .filter(|(name, method)| name == destination && method == "close")
                        .count()
                        + 1
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn password_store_import_and_native_round_trip_preserve_all_original_choices() {
        let defaults = crate::storage::Settings::default();
        assert_eq!(defaults.password_store, Backend::SecretService);
        for (name, expected) in [
            ("gnome_libsecret", Backend::SecretService),
            ("gnome-libsecret", Backend::SecretService),
            ("kwallet", Backend::KWallet4),
            ("kwallet5", Backend::KWallet5),
            ("kwallet6", Backend::KWallet6),
        ] {
            let preview = crate::migration::preview(
                &defaults,
                &serde_json::to_vec(&serde_json::json!({"general":{"passwordStore":name}}))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(preview.settings.password_store, expected);
            assert!(preview.omitted.is_empty());
            let restored: crate::storage::Settings =
                serde_json::from_slice(&serde_json::to_vec(&preview.settings).unwrap()).unwrap();
            assert_eq!(restored.password_store, expected);
        }
        assert!(
            crate::migration::preview(&defaults, br#"{"general":{"passwordStore":"plaintext"}}"#)
                .is_err()
        );
    }
}
