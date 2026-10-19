//! Regenerable disk caches, kept separate from imported lyrics and local artwork.
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn versioned(key: &str, revision: u64) -> String {
    if revision == 0 {
        key.to_owned()
    } else {
        format!("{key}-cache-{revision}")
    }
}

pub fn clear_disk(root: &Path, protected: &[PathBuf]) -> Result<usize> {
    let mut count = 0;
    for directory in [
        "covers",
        "censored-covers",
        "player-backgrounds",
        "detail-backgrounds",
        "waveforms",
        "lyrics/cache",
    ] {
        let directory = root.join(directory);
        let metadata = match std::fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("Could not inspect the native cache"),
        };
        if metadata.is_symlink() {
            std::fs::remove_file(&directory).context("Could not remove the cache link")?;
            count += 1;
            continue;
        }
        for file in std::fs::read_dir(&directory).context("Could not read the native cache")? {
            let file = file?;
            let path = file.path();
            if protected.contains(&path) {
                continue;
            }
            if file.file_type()?.is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_file(&path)?;
            }
            count += 1;
        }
    }
    Ok(count)
}

pub fn discard_stale(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::debug!("Could not remove an obsolete cache entry: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disk_cache_clear_preserves_overrides_radio_artwork_preferences_and_other_user_files() {
        let root = std::env::temp_dir().join(format!("feishin-cache-{}", uuid::Uuid::new_v4()));
        let protected = root.join("covers/local-radio.img");
        for file in [
            "covers/album.img",
            "covers/local-radio.img",
            "censored-covers/album.png",
            "player-backgrounds/player.png",
            "detail-backgrounds/album.png",
            "waveforms/track.json",
            "lyrics/cache/track.json",
            "lyrics/overrides/track.json",
            "themes/custom.json",
            "state.json",
            "custom.css",
            "downloads/album.zip",
        ] {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file).unwrap();
        }
        assert_eq!(
            clear_disk(&root, std::slice::from_ref(&protected)).unwrap(),
            6
        );
        assert_eq!(
            std::fs::read_to_string(&protected).unwrap(),
            "covers/local-radio.img"
        );
        for file in [
            "lyrics/overrides/track.json",
            "themes/custom.json",
            "state.json",
            "custom.css",
            "downloads/album.zip",
        ] {
            assert_eq!(std::fs::read_to_string(root.join(file)).unwrap(), file);
        }
        assert_eq!(clear_disk(&root, &[protected]).unwrap(), 0);
        assert_eq!(versioned("track", 0), "track");
        assert_eq!(versioned("track", 7), "track-cache-7");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn cache_links_do_not_delete_their_targets() {
        use std::os::unix::fs::symlink;
        let root =
            std::env::temp_dir().join(format!("feishin-cache-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("outside")).unwrap();
        std::fs::write(root.join("outside/keep"), "user data").unwrap();
        symlink(root.join("outside"), root.join("covers")).unwrap();
        std::fs::create_dir_all(root.join("waveforms")).unwrap();
        symlink(root.join("outside"), root.join("waveforms/linked")).unwrap();
        assert_eq!(clear_disk(&root, &[]).unwrap(), 2);
        assert_eq!(
            std::fs::read_to_string(root.join("outside/keep")).unwrap(),
            "user data"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
