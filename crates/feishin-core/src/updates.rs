use anyhow::{Context, Result, ensure};
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const FEED: &str = "https://api.github.com/repos/jeffvli/feishin/releases?per_page=100";
const RELEASE_PREFIX: &str = "https://github.com/jeffvli/feishin/releases/";
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
const MAX_EXPANDED: u64 = 1024 * 1024 * 1024;

pub fn disabled_by_environment() -> bool {
    environment_disabled(std::env::var_os("DISABLE_AUTO_UPDATES").as_deref())
}
fn environment_disabled(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Latest,
    Beta,
    Alpha,
}
impl Channel {
    pub const ALL: [Self; 3] = [Self::Latest, Self::Beta, Self::Alpha];
    pub fn label(self) -> &'static str {
        match self {
            Self::Latest => "Latest",
            Self::Beta => "Beta",
            Self::Alpha => "Alpha",
        }
    }
    fn accepts(self, version: &Version) -> bool {
        version.pre.is_empty()
            || match self {
                Self::Latest => false,
                Self::Beta => version.pre.as_str().split('.').next() == Some("beta"),
                Self::Alpha => matches!(
                    version.pre.as_str().split('.').next(),
                    Some("alpha" | "beta")
                ),
            }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub channel: Channel,
    pub automatic: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            channel: Channel::Latest,
            automatic: true,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
    state: String,
}
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    draft: bool,
    assets: Vec<Asset>,
}
#[derive(Clone, Debug)]
pub struct Available {
    pub version: String,
    pub notes: String,
    pub page: String,
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}
fn client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(concat!("Feishin-native/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error("Too many update redirects");
            }
            if trusted_url(attempt.url().as_str()) {
                attempt.follow()
            } else {
                attempt.error("Untrusted update redirect")
            }
        }))
        .build()?)
}
fn trusted_url(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return false;
    }
    #[cfg(test)]
    if url.scheme() == "http" && url.host_str() == Some("127.0.0.1") {
        return true;
    }
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "api.github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}
pub fn asset_name() -> String {
    let suffix = if cfg!(target_os = "linux") {
        "tar.gz"
    } else {
        "zip"
    };
    format!(
        "feishin-native-{}-{}.{suffix}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}
fn select(
    releases: Vec<Release>,
    current: &str,
    channel: Channel,
    name: &str,
) -> Result<Option<Available>> {
    let current = Version::parse(current)?;
    let mut found = None;
    let mut best = current;
    for release in releases.into_iter().filter(|release| !release.draft) {
        let Ok(version) = Version::parse(
            release
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&release.tag_name),
        ) else {
            continue;
        };
        if version <= best
            || !channel.accepts(&version)
            || !release.html_url.starts_with(RELEASE_PREFIX)
        {
            continue;
        }
        let Some(asset) = release
            .assets
            .into_iter()
            .find(|asset| asset.name == name && asset.state == "uploaded")
        else {
            continue;
        };
        let Some(digest) = asset
            .digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("sha256:"))
        else {
            continue;
        };
        if digest.len() != 64
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !(1..=MAX_ARCHIVE).contains(&asset.size)
            || !asset
                .browser_download_url
                .starts_with(&format!("{RELEASE_PREFIX}download/"))
            || !trusted_url(&asset.browser_download_url)
        {
            continue;
        }
        best = version.clone();
        found = Some(Available {
            version: version.to_string(),
            notes: release
                .body
                .unwrap_or_default()
                .chars()
                .take(100_000)
                .collect(),
            page: release.html_url,
            name: asset.name,
            url: asset.browser_download_url,
            size: asset.size,
            sha256: digest.to_ascii_lowercase(),
        });
    }
    Ok(found)
}
pub fn check(config: &Config) -> Result<Option<Available>> {
    check_feed(
        FEED,
        env!("CARGO_PKG_VERSION"),
        config.channel,
        &asset_name(),
    )
}
fn check_feed(
    feed: &str,
    current: &str,
    channel: Channel,
    name: &str,
) -> Result<Option<Available>> {
    let response = client()?
        .get(feed)
        .header("Accept", "application/vnd.github+json")
        .send()?
        .error_for_status()?;
    let mut bytes = vec![];
    response.take(8_000_001).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 8_000_000, "Update response is too large");
    select(
        serde_json::from_slice(&bytes).context("Invalid update response")?,
        current,
        channel,
        name,
    )
}
struct Partial(PathBuf);
impl Drop for Partial {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub fn download(
    update: &Available,
    root: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf> {
    ensure!(trusted_url(&update.url), "Invalid update URL");
    ensure!(
        update.name == asset_name() && (1..=MAX_ARCHIVE).contains(&update.size),
        "Update package does not match this platform"
    );
    ensure!(
        Version::parse(&update.version).is_ok() && update.version.len() < 100,
        "Invalid update version"
    );
    ensure!(
        update.sha256.len() == 64 && update.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Missing update checksum"
    );
    let directory = root.join(&update.version);
    fs::create_dir_all(&directory)?;
    let destination = directory.join(&update.name);
    if destination.is_file() && checksum(&destination)? == update.sha256.to_ascii_lowercase() {
        progress(update.size, update.size);
        return Ok(destination);
    }
    let partial = Partial(directory.join(format!(".download-{}.part", uuid::Uuid::new_v4())));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial.0)?;
    let mut response = client()?.get(&update.url).send()?.error_for_status()?;
    if let Some(size) = response.content_length() {
        ensure!(
            size == update.size,
            "Update size does not match its release metadata"
        );
    }
    let mut digest = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count += read as u64;
        ensure!(count <= update.size, "Update exceeds its release size");
        digest.update(&buffer[..read]);
        output.write_all(&buffer[..read])?;
        progress(count, update.size);
    }
    ensure!(
        count == update.size
            && format!("{:x}", digest.finalize()) == update.sha256.to_ascii_lowercase(),
        "Update checksum does not match its release metadata"
    );
    output.sync_all()?;
    drop(output);
    let destination = directory.join(&update.name);
    fs::rename(&partial.0, &destination)?;
    Ok(destination)
}
fn safe_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "Invalid update archive path"
    );
    Ok(())
}
fn unpack(archive: &Path, root: &Path) -> Result<()> {
    fs::create_dir(root)?;
    let mut expanded = 0_u64;
    if archive
        .extension()
        .is_some_and(|extension| extension == "zip")
    {
        let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
        ensure!(zip.len() <= 10_000, "Update contains too many files");
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index)?;
            let path = entry
                .enclosed_name()
                .context("Invalid update archive path")?;
            safe_path(&path)?;
            ensure!(
                entry
                    .unix_mode()
                    .is_none_or(|mode| mode & 0o170000 != 0o120000),
                "Update archive contains a symbolic link"
            );
            let destination = root.join(path);
            if entry.is_dir() {
                fs::create_dir_all(destination)?;
                continue;
            }
            expanded = expanded
                .checked_add(entry.size())
                .context("Invalid update size")?;
            ensure!(expanded <= MAX_EXPANDED, "Expanded update is too large");
            fs::create_dir_all(destination.parent().context("Invalid update path")?)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)?;
            let size = entry.size();
            let count = std::io::copy(&mut entry.by_ref().take(size + 1), &mut output)?;
            ensure!(count == entry.size(), "Truncated update archive");
            #[cfg(unix)]
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&destination, fs::Permissions::from_mode(mode & 0o777))?;
            }
        }
    } else {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(File::open(archive)?));
        for (index, entry) in tar.entries()?.enumerate() {
            ensure!(index < 10_000, "Update contains too many files");
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            safe_path(&path)?;
            ensure!(
                entry.header().entry_type().is_file() || entry.header().entry_type().is_dir(),
                "Update archive contains a link or special file"
            );
            expanded = expanded
                .checked_add(entry.size())
                .context("Invalid update size")?;
            ensure!(expanded <= MAX_EXPANDED, "Expanded update is too large");
            ensure!(entry.unpack_in(root)?, "Invalid update archive path");
        }
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct InstallJob {
    staged: PathBuf,
    target: PathBuf,
    checksum: String,
    parent: u32,
    restart: bool,
}
#[derive(Clone, Debug)]
pub struct Prepared {
    pub job: PathBuf,
    pub helper: PathBuf,
}
fn executable(package: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        package.join("Contents/MacOS/feishin")
    } else {
        package.to_owned()
    }
}
fn checksum(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub fn prepare(archive: &Path, current: &Path) -> Result<Prepared> {
    let root = archive
        .parent()
        .context("Update package has no directory")?
        .join(format!("install-{}", uuid::Uuid::new_v4()));
    let unpacked = root.join("package");
    fs::create_dir(&root)?;
    let result = (|| {
        unpack(archive, &unpacked)?;
        let (staged, target) = if cfg!(target_os = "macos") {
            let target = current
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .context("Cannot locate application bundle")?;
            ensure!(
                target
                    .extension()
                    .is_some_and(|extension| extension == "app")
                    && current == executable(target),
                "Install updates from a packaged application"
            );
            let staged = unpacked.join("Feishin.app");
            ensure!(
                staged
                    .join("Contents/MacOS/feishin-audio-capture")
                    .is_file()
                    && staged.join("Contents/Info.plist").is_file(),
                "Incomplete native application bundle"
            );
            (staged, target.to_owned())
        } else if cfg!(target_os = "windows") {
            (unpacked.join("feishin.exe"), current.to_owned())
        } else {
            (
                unpacked.join("feishin-linux/bin/feishin"),
                current.to_owned(),
            )
        };
        ensure!(
            executable(&staged).is_file(),
            "Native executable is missing from the update package"
        );
        let helper = root.join(if cfg!(windows) {
            "installer.exe"
        } else {
            "installer"
        });
        fs::copy(current, &helper)?;
        let job = root.join("job.json");
        crate::storage::atomic_write(
            &job,
            &serde_json::to_vec(&InstallJob {
                staged,
                target,
                checksum: checksum(current)?,
                parent: std::process::id(),
                restart: false,
            })?,
        )?;
        Ok(Prepared { job, helper })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&root);
    }
    result
}
pub fn launch(prepared: &Prepared, restart: bool) -> Result<()> {
    let mut job: InstallJob = serde_json::from_slice(&fs::read(&prepared.job)?)?;
    job.restart = restart;
    crate::storage::atomic_write(&prepared.job, &serde_json::to_vec(&job)?)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(prepared.job.with_extension("log"))?;
    let mut command = Command::new(&prepared.helper);
    command
        .arg("--apply-native-update")
        .arg(&prepared.job)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .context("Cannot start native update installer")?;
    Ok(())
}
pub(crate) fn parent_running(pid: u32) -> Result<bool> {
    #[cfg(unix)]
    {
        Ok(unsafe { libc::kill(pid as libc::pid_t, 0) } == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
    }
    #[cfg(windows)]
    {
        use windows::Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        };
        unsafe {
            let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return Ok(false);
            };
            let mut code = 0;
            let result = GetExitCodeProcess(process, &mut code);
            let _ = CloseHandle(process);
            result?;
            Ok(code == 259)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        anyhow::bail!("Native updating is unsupported on this platform")
    }
}
fn copy_package(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    ensure!(!metadata.is_symlink(), "Unexpected link in staged update");
    if metadata.is_dir() {
        fs::create_dir(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_package(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else {
        ensure!(metadata.is_file(), "Unexpected file in staged update");
        fs::copy(source, destination)?;
    }
    fs::set_permissions(destination, metadata.permissions())?;
    Ok(())
}
fn apply(job: &InstallJob) -> Result<()> {
    ensure!(
        job.target.is_absolute() && job.staged.is_absolute(),
        "Invalid installer paths"
    );
    ensure!(
        checksum(&executable(&job.target))? == job.checksum,
        "Installed application changed after this update was downloaded"
    );
    let parent = job
        .target
        .parent()
        .context("Invalid installed application path")?;
    let replacement = parent.join(format!(".feishin-update-{}", uuid::Uuid::new_v4()));
    let backup = parent.join(format!(".feishin-backup-{}", uuid::Uuid::new_v4()));
    if let Err(error) = copy_package(&job.staged, &replacement) {
        let _ = if replacement.is_dir() {
            fs::remove_dir_all(&replacement)
        } else {
            fs::remove_file(&replacement)
        };
        return Err(error.context("Cannot stage update beside the installed application"));
    }
    if let Err(error) = fs::rename(&job.target, &backup) {
        let _ = if replacement.is_dir() {
            fs::remove_dir_all(&replacement)
        } else {
            fs::remove_file(&replacement)
        };
        return Err(anyhow::Error::new(error).context("Cannot replace the installed application"));
    }
    if let Err(error) = fs::rename(&replacement, &job.target) {
        fs::rename(&backup, &job.target).context("Cannot restore the previous application")?;
        return Err(anyhow::Error::new(error)
            .context("Cannot install update; restored the previous application"));
    }
    if job.restart {
        if cfg!(target_os = "macos") {
            Command::new("/usr/bin/open").arg(&job.target).spawn()?;
        } else {
            Command::new(&job.target)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }
    }
    let _ = if backup.is_dir() {
        fs::remove_dir_all(backup)
    } else {
        fs::remove_file(backup)
    };
    Ok(())
}
pub fn run_installer(path: &Path) -> Result<()> {
    let bytes = fs::read(path)?;
    ensure!(bytes.len() <= 32_000, "Invalid installer job");
    let job: InstallJob = serde_json::from_slice(&bytes)?;
    ensure!(
        job.parent > 0 && job.parent != std::process::id(),
        "Invalid installer parent process"
    );
    let start = Instant::now();
    while parent_running(job.parent)? {
        ensure!(
            start.elapsed() < Duration::from_secs(120),
            "Application did not exit for updating"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    apply(&job)?;
    println!("Native update installed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn original_update_environment_override_treats_every_nonempty_value_as_disabled() {
        assert!(!environment_disabled(None));
        assert!(!environment_disabled(Some(std::ffi::OsStr::new(""))));
        for value in ["true", "false", "1", "0", "disabled"] {
            assert!(environment_disabled(Some(std::ffi::OsStr::new(value))));
        }
    }
    fn release(version: &str, name: &str) -> serde_json::Value {
        json!({"tag_name":format!("v{version}"),"html_url":format!("{RELEASE_PREFIX}tag/v{version}"),"body":"Release notes","draft":false,
            "assets":[{"name":name,"state":"uploaded","size":3,"digest":format!("sha256:{}", "a".repeat(64)),"browser_download_url":format!("{RELEASE_PREFIX}download/v{version}/{name}")}]})
    }
    fn serve(body: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/update", server.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = vec![];
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 10_000);
            }
            assert!(
                !String::from_utf8_lossy(&request)
                    .to_ascii_lowercase()
                    .contains("authorization:")
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
        });
        (url, handle)
    }
    #[test]
    fn channels_versions_and_native_platform_selection() {
        let name = asset_name();
        let values = json!([
            release("1.19.0-alpha.1", &name),
            release("1.17.9", &name),
            release("1.18.0-beta.2", &name),
            release("9.0.0", "Feishin-electron.zip"),
            release("1.17.10", &name)
        ]);
        let select_channel = |channel| {
            select(
                serde_json::from_value(values.clone()).unwrap(),
                "1.17.0",
                channel,
                &name,
            )
            .unwrap()
            .unwrap()
            .version
        };
        assert_eq!(select_channel(Channel::Latest), "1.17.10");
        assert_eq!(select_channel(Channel::Beta), "1.18.0-beta.2");
        assert_eq!(select_channel(Channel::Alpha), "1.19.0-alpha.1");
        let mut invalid = release("2.0.0", &name);
        invalid["assets"][0]["digest"] = json!(null);
        assert!(
            select(
                serde_json::from_value(json!([invalid])).unwrap(),
                "1.17.0",
                Channel::Alpha,
                &name
            )
            .unwrap()
            .is_none()
        );
        assert!(!trusted_url(
            "https://github.com.evil.invalid/releases/download/app.zip"
        ));
        assert!(!trusted_url(
            "https://user:secret@github.com/releases/download/app.zip"
        ));
        let (url, server) = serve(serde_json::to_vec(&values).unwrap());
        assert_eq!(
            check_feed(&url, "1.17.0", Channel::Latest, &name)
                .unwrap()
                .unwrap()
                .version,
            "1.17.10"
        );
        server.join().unwrap();
    }
    #[test]
    fn downloads_verify_integrity_and_remove_failed_partial_files() {
        let root =
            std::env::temp_dir().join(format!("feishin-update-download-{}", uuid::Uuid::new_v4()));
        let data = b"verified native archive".to_vec();
        let (url, server) = serve(data.clone());
        let mut update = Available {
            version: "1.18.0".into(),
            notes: String::new(),
            page: String::new(),
            name: asset_name(),
            url,
            size: data.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&data)),
        };
        let mut progress = vec![];
        let archive =
            download(&update, &root, |count, total| progress.push((count, total))).unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(&archive).unwrap(), data);
        assert_eq!(progress.last(), Some(&(update.size, update.size)));
        let mut corrupted = data.clone();
        corrupted[0] = b'!';
        let (url, server) = serve(corrupted);
        update.url = url;
        update.version = "1.18.1".into();
        assert!(download(&update, &root, |_, _| {}).is_err());
        server.join().unwrap();
        assert_eq!(fs::read(&archive).unwrap(), data);
        assert_eq!(fs::read_dir(root.join("1.18.0")).unwrap().count(), 1);
        assert_eq!(fs::read_dir(root.join("1.18.1")).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn archive_paths_and_installer_preserve_existing_application_on_failure() {
        let root =
            std::env::temp_dir().join(format!("feishin-update-install-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let archive = root.join("malicious.zip");
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        zip.start_file("../escaped", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"bad").unwrap();
        zip.finish().unwrap();
        assert!(unpack(&archive, &root.join("unpacked")).is_err());
        assert!(!root.join("escaped").exists());
        let target = root.join(if cfg!(target_os = "macos") {
            "Feishin.app"
        } else {
            "feishin"
        });
        let staged = root.join(if cfg!(target_os = "macos") {
            "New.app"
        } else {
            "new"
        });
        for (package, contents) in [(&target, b"old"), (&staged, b"new")] {
            let file = executable(package);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, contents).unwrap();
        }
        let mut job = InstallJob {
            target: target.clone(),
            staged: staged.clone(),
            checksum: checksum(&executable(&target)).unwrap(),
            parent: 1,
            restart: false,
        };
        job.checksum = "wrong".into();
        assert!(apply(&job).is_err());
        assert_eq!(fs::read(executable(&target)).unwrap(), b"old");
        job.checksum = checksum(&executable(&target)).unwrap();
        job.staged = root.join("missing");
        assert!(apply(&job).is_err());
        assert_eq!(fs::read(executable(&target)).unwrap(), b"old");
        job.staged = staged;
        apply(&job).unwrap();
        assert_eq!(fs::read(executable(&target)).unwrap(), b"new");
        assert!(!fs::read_dir(&root).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".feishin-")
        }));
        fs::remove_dir_all(root).unwrap();
    }
}
