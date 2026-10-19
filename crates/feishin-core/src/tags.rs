use anyhow::{Context, Result, ensure};
use lofty::picture::Picture;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Config {
    pub multi_value: bool,
    pub autocomplete_source: String,
    pub custom_values: Vec<String>,
}
pub const KNOWN_FIELDS: &[&str] = &[
    "ACOUSTID_FINGERPRINT",
    "ACOUSTID_ID",
    "ALBUM",
    "ALBUMARTIST",
    "ALBUMARTISTSORT",
    "ALBUMSORT",
    "ARTIST",
    "ARTISTS",
    "ARTISTSORT",
    "ASIN",
    "BARCODE",
    "BPM",
    "CATALOGNUMBER",
    "COMMENT",
    "COMPILATION",
    "COMPOSER",
    "COMPOSERSORT",
    "CONDUCTOR",
    "COPYRIGHT",
    "DATE",
    "DISCNUMBER",
    "DISCTOTAL",
    "ENCODEDBY",
    "ENCODING",
    "GENRE",
    "GROUPING",
    "INVOLVEDPEOPLE",
    "ISRC",
    "ITUNNORM",
    "LABEL",
    "LANGUAGE",
    "LYRICIST",
    "LYRICS",
    "MEDIA",
    "MOOD",
    "MUSICBRAINZ_ALBUMARTISTID",
    "MUSICBRAINZ_ALBUMID",
    "MUSICBRAINZ_ARTISTID",
    "MUSICBRAINZ_RELEASEGROUPID",
    "MUSICBRAINZ_RELEASETRACKID",
    "MUSICBRAINZ_TRACKID",
    "MUSICBRAINZ_WORKID",
    "ORIGINALALBUM",
    "ORIGINALARTIST",
    "ORIGINALDATE",
    "ORIGINALYEAR",
    "PODCASTID",
    "PODCASTURL",
    "PRODUCER",
    "PUBLISHER",
    "RADIOSTATIONOWNER",
    "RELEASECOUNTRY",
    "RELEASESTATUS",
    "RELEASETYPE",
    "REMIXER",
    "REPLAYGAIN_ALBUM_GAIN",
    "REPLAYGAIN_ALBUM_PEAK",
    "REPLAYGAIN_TRACK_GAIN",
    "REPLAYGAIN_TRACK_PEAK",
    "SCRIPT",
    "SUBTITLE",
    "TITLE",
    "TITLESORT",
    "TRACKNUMBER",
    "TRACKTOTAL",
    "WORK",
];

pub fn canonical_name(name: &str) -> String {
    let name = name.trim_matches(tag_whitespace).to_uppercase();
    match name.as_str() {
        "TOTALTRACKS" => "TRACKTOTAL",
        "TOTALDISCS" => "DISCTOTAL",
        "REMIXEDBY" => "REMIXER",
        "MUSICBRAINZARTISTID" => "MUSICBRAINZ_ARTISTID",
        "MUSICBRAINZRELEASEID" => "MUSICBRAINZ_ALBUMID",
        "MUSICBRAINZTRACKID" => "MUSICBRAINZ_TRACKID",
        "MUSICBRAINZRELEASEGROUPID" => "MUSICBRAINZ_RELEASEGROUPID",
        "REPLAYGAINTRACKGAIN" => "REPLAYGAIN_TRACK_GAIN",
        "REPLAYGAINTRACKPEAK" => "REPLAYGAIN_TRACK_PEAK",
        "REPLAYGAINALBUMGAIN" => "REPLAYGAIN_ALBUM_GAIN",
        "REPLAYGAINALBUMPEAK" => "REPLAYGAIN_ALBUM_PEAK",
        "ACOUSTIDFINGERPRINT" => "ACOUSTID_FINGERPRINT",
        "ACOUSTIDID" => "ACOUSTID_ID",
        "APPLESOUNDCHECK" => "ITUNNORM",
        "MUSICBRAINZRELEASEARTISTID" => "MUSICBRAINZ_ALBUMARTISTID",
        "MUSICBRAINZWORKID" => "MUSICBRAINZ_WORKID",
        "MUSICBRAINZRELEASETRACKID" => "MUSICBRAINZ_RELEASETRACKID",
        _ => return name,
    }
    .into()
}
pub fn validate_field_name(name: &str) -> Result<()> {
    let name = name.trim_matches(tag_whitespace);
    ensure!(
        !name.is_empty() && name.len() <= 256 && !name.contains(['\0', '\n', '\r']),
        "Invalid tag name"
    );
    ensure!(!name.contains('='), "Tag key cannot contain '='");
    ensure!(name.is_ascii(), "Tag key must use ASCII characters only");
    Ok(())
}

fn tag_whitespace(c: char) -> bool {
    // Match JavaScript trim in the original tag editor, including the BOM.
    c != '\u{0085}' && (c.is_whitespace() || c == '\u{feff}')
}
pub fn default_configs() -> BTreeMap<String, Config> {
    [
        ("ALBUMARTIST", false, "serverArtists"),
        ("ALBUMARTISTS", true, "serverArtists"),
        ("ALBUMARTISTSORT", false, "serverArtists"),
        ("ALBUMARTISTSSORT", false, "serverArtists"),
        ("ARTIST", true, "serverArtists"),
        ("ARTISTS", true, "serverArtists"),
        ("ARTISTSORT", false, "serverArtists"),
        ("ARTISTSSORT", false, "serverArtists"),
        ("GENRE", true, "serverGenres"),
    ]
    .into_iter()
    .map(|(name, multi, source)| {
        (
            name.into(),
            Config {
                multi_value: multi,
                autocomplete_source: source.into(),
                custom_values: Vec::new(),
            },
        )
    })
    .collect()
}
pub fn validate_configs(configs: &BTreeMap<String, Config>) -> Result<()> {
    ensure!(configs.len() <= 500, "Too many tag field configurations");
    for (name, config) in configs {
        ensure!(
            !name.is_empty() && name.len() <= 256 && !name.contains(['=', '\0', '\n']),
            "Invalid tag field configuration name"
        );
        ensure!(
            matches!(
                config.autocomplete_source.as_str(),
                "" | "none" | "serverArtists" | "serverGenres" | "custom"
            ) || config
                .autocomplete_source
                .strip_prefix("tag:")
                .is_some_and(|name| {
                    !name.is_empty() && name.len() <= 256 && !name.contains(['\0', '\n'])
                }),
            "Unknown tag suggestion source"
        );
        ensure!(
            config.custom_values.len() <= 500
                && config
                    .custom_values
                    .iter()
                    .all(|value| value.len() <= 4096 && !value.contains('\0')),
            "Invalid custom tag suggestions"
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldType {
    Text,
    Textarea,
    Number,
    Boolean,
}
pub fn field_type(name: &str) -> FieldType {
    match canonical_name(name).as_str() {
        "COMPILATION" => FieldType::Boolean,
        "BPM" | "ORIGINALYEAR" => FieldType::Number,
        "LYRICS" | "COMMENT" | "ACOUSTID_FINGERPRINT" => FieldType::Textarea,
        _ => FieldType::Text,
    }
}
pub fn empty_edit(name: &str, values: &[String]) -> bool {
    field_type(name) != FieldType::Boolean
        && (values.is_empty()
            || values
                .iter()
                .any(|value| value.trim_matches(tag_whitespace).is_empty()))
}
pub fn validate_field_value(name: &str, value: &str, multi: bool) -> Result<()> {
    if value.is_empty() || multi {
        return Ok(());
    }
    match field_type(name) {
        FieldType::Number => ensure!(
            value.trim().parse::<f64>().is_ok_and(f64::is_finite),
            "{name} must contain a finite number"
        ),
        FieldType::Boolean => ensure!(matches!(value, "0" | "1"), "{name} must be 0 or 1"),
        _ => (),
    }
    Ok(())
}

pub type Fields = BTreeMap<String, Vec<String>>;

#[derive(Clone, Debug, Default)]
pub struct Suggestions {
    pub custom: Vec<String>,
    pub server: Vec<String>,
}

pub fn source_label(name: &str) -> String {
    crate::smart::fields()
        .iter()
        .find(|field| field.value == name)
        .map_or_else(
            || crate::i18n::title_case(name),
            |field| field.label.clone(),
        )
}

pub fn suggestions(custom: &[String], server: &[String]) -> Suggestions {
    let mut custom = custom.to_vec();
    custom.sort_by(|a, b| crate::i18n::compare(a, b, false));
    let existing = custom
        .iter()
        .map(|value| value.to_lowercase())
        .collect::<std::collections::HashSet<_>>();
    let mut seen = std::collections::HashSet::new();
    let server = server
        .iter()
        .filter(|value| {
            !value.is_empty()
                && !existing.contains(&value.to_lowercase())
                && seen.insert(value.as_str())
        })
        .cloned()
        .collect();
    Suggestions { custom, server }
}

#[derive(Clone, Debug)]
pub struct Metadata {
    pub path: PathBuf,
    pub fields: Fields,
    pub artwork: Option<Vec<u8>>,
}

#[derive(Clone, Default)]
pub enum Artwork {
    #[default]
    Keep,
    Remove,
    Replace(Vec<u8>),
}

#[derive(Clone, Default)]
pub struct Patch {
    // Empty values remove the field. Fields absent from the patch stay untouched.
    pub fields: Fields,
    pub artwork: Artwork,
}

pub fn read(path: &Path) -> Result<Metadata> {
    crate::tags_taglib::read(path)
}

pub fn merged(files: &[Metadata]) -> BTreeMap<String, Option<Vec<String>>> {
    fn display_values<'a>(
        name: &str,
        values: &'a [String],
    ) -> Option<std::borrow::Cow<'a, [String]>> {
        let values = if name == "LYRICS" {
            std::borrow::Cow::Owned(vec![values.join("\n\n")])
        } else {
            std::borrow::Cow::Borrowed(values)
        };
        values
            .first()
            .is_some_and(|value| !value.is_empty())
            .then_some(values)
    }
    let mut result = BTreeMap::new();
    for file in files {
        for (key, values) in &file.fields {
            if display_values(key, values).is_none() {
                continue;
            }
            result.entry(key.clone()).or_insert_with(|| {
                let first = display_values(key, files.first()?.fields.get(key)?)?;
                files
                    .iter()
                    .all(|file| {
                        file.fields
                            .get(key)
                            .and_then(|values| display_values(key, values))
                            .as_ref()
                            == Some(&first)
                    })
                    .then(|| first.into_owned())
            });
        }
    }
    result
}

pub fn artwork_file(path: &Path) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(20_000_001)
        .read_to_end(&mut data)?;
    ensure!(data.len() <= 20_000_000, "Artwork is too large");
    artwork_mime(&data)?;
    Ok(data)
}

pub fn artwork_mime(data: &[u8]) -> Result<String> {
    ensure!(data.len() <= 20_000_000, "Artwork is too large");
    // Lofty's picture detection lacks WebP, which the original editor accepts.
    if data.len() >= 20 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        let length = u32::from_le_bytes(data[4..8].try_into()?) as usize;
        let chunk = u32::from_le_bytes(data[16..20].try_into()?) as usize;
        ensure!(
            length.checked_add(8) == Some(data.len())
                && chunk <= data.len() - 20
                && matches!(&data[12..16], b"VP8 " | b"VP8L" | b"VP8X"),
            "Invalid WebP artwork"
        );
        return Ok("image/webp".into());
    }
    let picture = Picture::from_reader(&mut std::io::Cursor::new(data))?;
    Ok(picture
        .mime_type()
        .context("Unknown artwork format")?
        .as_str()
        .to_owned())
}

pub fn write(path: &Path, patch: &Patch) -> Result<()> {
    let path = path.canonicalize()?;
    let original = fs::metadata(&path)?;
    ensure!(original.is_file(), "Select an audio file");
    ensure!(
        !original.permissions().readonly(),
        "Audio file is read-only"
    );
    let parent = path
        .parent()
        .context("Audio file has no parent directory")?;
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            extension.len() <= 16 && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .unwrap_or("audio");
    let temporary = parent.join(format!(
        ".feishin-tags-{}.{extension}",
        uuid::Uuid::new_v4()
    ));
    let result = (|| {
        let mut copy = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        std::io::copy(&mut fs::File::open(&path)?, &mut copy)?;
        copy.flush()?;
        copy.rewind()?;
        fs::set_permissions(&temporary, original.permissions())?;
        crate::tags_taglib::write(&temporary, patch)?;
        copy.sync_all()?;
        // A failed parse must never replace the original audio file.
        let saved = read(&temporary)?;
        for (name, values) in &patch.fields {
            // Some formats, including ASF, serialize multiple values in their
            // own order. Verify every value and its multiplicity, not that order.
            let mut actual = saved.fields.get(name).cloned().unwrap_or_default();
            let mut expected = values.clone();
            actual.sort();
            expected.sort();
            ensure!(
                actual == expected,
                "The file format could not preserve all values for {name}: {:?}; original audio was kept",
                saved.fields.get(name)
            );
        }
        match &patch.artwork {
            Artwork::Replace(data) => ensure!(
                saved.artwork.as_ref() == Some(data),
                "The format could not preserve cover artwork; original audio was kept"
            ),
            Artwork::Remove => ensure!(
                saved.artwork.is_none(),
                "The format could not remove cover artwork; original audio was kept"
            ),
            Artwork::Keep => {}
        }
        let current = fs::metadata(&path)?;
        ensure!(
            current.len() == original.len() && current.modified()? == original.modified()?,
            "Audio file changed during the edit; reload it before saving"
        );
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn write_batch(paths: &[PathBuf], patch: &Patch) -> Vec<(PathBuf, Result<()>)> {
    paths
        .iter()
        .map(|path| (path.clone(), write(path, patch)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::{
        config::WriteOptions,
        file::{AudioFile, TaggedFileExt},
        probe::Probe,
        tag::{ItemKey, Tag, TagType},
    };
    #[test]
    fn suggestion_groups_match_original() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/tag-suggestions-reference.json")).unwrap();
        for case in reference["labels"].as_array().unwrap() {
            assert_eq!(
                source_label(case["name"].as_str().unwrap()),
                case["expected"].as_str().unwrap()
            );
        }
        for case in reference["cases"].as_array().unwrap() {
            let custom: Vec<String> = serde_json::from_value(case["custom"].clone()).unwrap();
            let search = case["search"].as_str().unwrap();
            let server = match case["source"].as_str().unwrap() {
                "none" => vec![],
                "tag:mood" => reference["catalog"]["tags"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|tag| tag["name"] == "mood")
                    .unwrap()["options"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|item| item["name"].as_str())
                    .filter(|name| !name.is_empty() && name.to_lowercase().contains(search))
                    .take(25)
                    .map(str::to_owned)
                    .collect(),
                _ => serde_json::from_value(case["server"].clone()).unwrap(),
            };
            let actual = suggestions(&custom, &server);
            let mut groups = vec![];
            if !actual.custom.is_empty() {
                groups.push(serde_json::json!({"group":"Custom values", "items":actual.custom}));
            }
            if !actual.server.is_empty() {
                groups.push(serde_json::json!({"group":"Server", "items":actual.server}));
            }
            assert_eq!(
                serde_json::to_value(groups).unwrap(),
                case["expected"],
                "{case}"
            );
        }
    }

    #[test]
    fn editor_names_types_and_blank_edits_match_original() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/tag-editor-reference.json")).unwrap();
        let original_names = reference["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| field["tagName"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            KNOWN_FIELDS
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            original_names
        );
        for name in KNOWN_FIELDS {
            validate_field_name(name).unwrap();
        }
        for name in ["A=B", "日本語", "", "LINE\nBREAK", "NUL\0VALUE"] {
            assert!(validate_field_name(name).is_err());
        }
        for case in reference["resolve"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            assert_eq!(
                canonical_name(name),
                case["expected"].as_str().unwrap(),
                "{name:?}"
            );
        }
        for case in reference["merged"].as_array().unwrap() {
            let files = case["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|fields| Metadata {
                    path: PathBuf::new(),
                    fields: serde_json::from_value(fields.clone()).unwrap(),
                    artwork: None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::to_value(merged(&files)).unwrap(),
                case["expected"],
                "{case}"
            );
        }
        for case in reference["add"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let canonical = canonical_name(name);
            let added = (validate_field_name(name).is_ok()
                && !case["existing"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|name| name.as_str() == Some(&canonical)))
            .then_some(canonical);
            assert_eq!(added.as_deref(), case["expected"].as_str(), "{case}");
        }
        for field in reference["fields"].as_array().unwrap() {
            let expected = match field["type"].as_str().unwrap() {
                "boolean" => FieldType::Boolean,
                "number" => FieldType::Number,
                "textarea" => FieldType::Textarea,
                "string" => FieldType::Text,
                other => panic!("Unknown original field type: {other}"),
            };
            for name in ["key", "tagName"] {
                assert_eq!(
                    field_type(field[name].as_str().unwrap()),
                    expected,
                    "{field}"
                );
            }
        }
        for case in reference["validation"].as_array().unwrap() {
            let values = if let Some(values) = case["value"].as_array() {
                values
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned())
                    .collect()
            } else {
                vec![case["value"].as_str().unwrap().to_owned()]
            };
            assert_eq!(
                empty_edit(case["key"].as_str().unwrap(), &values),
                !case["expected"].as_array().unwrap().is_empty(),
                "{case}"
            );
        }
    }
    #[test]
    fn audio_formats_keep_decoded_audio_and_unedited_metadata_after_tag_changes() {
        use std::process::Command;
        let ffmpeg = std::env::var("FEISHIN_TEST_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
        if !Command::new(&ffmpeg)
            .arg("-version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            assert!(
                std::env::var_os("FEISHIN_TEST_FFMPEG").is_none(),
                "Requested FFmpeg is unavailable"
            );
            return;
        }
        let dir =
            std::env::temp_dir().join(format!("feishin-format-tags-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        use base64::Engine;
        // Synthetic 16x16 red WebP. No optional image encoder is needed to run this test.
        let webp = base64::engine::general_purpose::STANDARD.decode("UklGRjwAAABXRUJQVlA4IDAAAADQAQCdASoQABAAAgA0JaACdLoB+AADsAD+8Oj3/yC5YXXI1/8gP+QH/ID/+PIAAAA=").unwrap();
        assert_eq!(artwork_mime(&webp).unwrap(), "image/webp");
        assert!(artwork_mime(&webp[..webp.len() - 1]).is_err());
        let mut fixtures = Vec::new();
        for (extension, codec) in [
            ("flac", "flac"),
            ("mp3", "libmp3lame"),
            ("ogg", "vorbis"),
            ("m4a", "aac"),
            ("alac.m4a", "alac"),
            ("aiff", "pcm_s16be"),
            ("opus", "libopus"),
            ("wv", "wavpack"),
            ("wma", "wmav2"),
            ("aac", "aac"),
            ("mka", "flac"),
            ("tta", "tta"),
        ] {
            let path = dir.join(format!("fixture.{extension}"));
            let result = Command::new(&ffmpeg)
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=440:duration=0.25:sample_rate=44100",
                    "-ac",
                    "2",
                    "-strict",
                    "-2",
                    "-metadata",
                    "album=Keep album",
                    "-c:a",
                    codec,
                ])
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{extension}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            fixtures.push((extension, path));
        }
        for (extension, filename) in [("dsf", "empty10ms.dsf"), ("dff", "empty10ms.dff")] {
            let path = dir.join(format!("fixture.{extension}"));
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../vendor/taglib/tests/data")
                    .join(filename),
                &path,
            )
            .unwrap();
            fixtures.push((extension, path));
        }
        for (extension, path) in fixtures {
            eprintln!("Metadata round trip: {extension}");
            let decode = || {
                let output = Command::new(&ffmpeg)
                    .args(["-v", "error", "-i"])
                    .arg(&path)
                    .args(["-f", "s16le", "-c:a", "pcm_s16le", "-"])
                    .output()
                    .unwrap();
                assert!(output.status.success());
                output.stdout
            };
            let before = decode();
            write(
                &path,
                &Patch {
                    fields: BTreeMap::from([("ALBUM".into(), vec!["Keep album".into()])]),
                    ..Default::default()
                },
            )
            .unwrap_or_else(|error| panic!("{extension} initial tags: {error}"));
            assert_eq!(
                decode(),
                before,
                "{extension} audio changed while creating initial metadata"
            );
            assert!(!before.is_empty());
            let artists = if extension == "wma" {
                vec!["First".into()]
            } else {
                vec!["First".into(), "Second".into()]
            };
            let mut fields = BTreeMap::from([
                ("TITLE".into(), vec!["New title".into()]),
                ("ARTIST".into(), artists.clone()),
                ("GENRE".into(), vec!["Rock".into(), "Pop".into()]),
            ]);
            // ASF's TagLib property map has a fixed vocabulary and one artist.
            // Reject unsupported edits without replacing the original file.
            if extension == "wma" {
                let original = fs::read(&path).unwrap();
                assert!(
                    write(
                        &path,
                        &Patch {
                            fields: BTreeMap::from([(
                                "MY_CUSTOM_TAG".into(),
                                vec!["Custom value".into()]
                            )]),
                            ..Default::default()
                        }
                    )
                    .is_err()
                );
                assert_eq!(fs::read(&path).unwrap(), original);
            } else {
                fields.insert("MY_CUSTOM_TAG".into(), vec!["Custom value".into()]);
            }
            write(
                &path,
                &Patch {
                    fields,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|error| panic!("{extension}: {error}"));
            let metadata = read(&path).unwrap();
            assert_eq!(metadata.fields["TITLE"], ["New title"], "{extension}");
            assert_eq!(metadata.fields["ALBUM"], ["Keep album"], "{extension}");
            assert_eq!(metadata.fields["ARTIST"], artists, "{extension}");
            let mut genres = metadata.fields["GENRE"].clone();
            genres.sort();
            assert_eq!(genres, ["Pop", "Rock"], "{extension}");
            if extension != "wma" {
                assert_eq!(
                    metadata.fields["MY_CUSTOM_TAG"],
                    ["Custom value"],
                    "{extension}"
                );
            }
            assert_eq!(decode(), before, "{extension} audio changed");
            use base64::Engine;
            let artwork = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5f8AAAAASUVORK5CYII=").unwrap();
            write(
                &path,
                &Patch {
                    artwork: Artwork::Replace(artwork.clone()),
                    ..Default::default()
                },
            )
            .unwrap_or_else(|error| panic!("{extension} cover: {error}"));
            assert_eq!(read(&path).unwrap().artwork, Some(artwork), "{extension}");
            assert_eq!(
                decode(),
                before,
                "{extension} audio changed after artwork replacement"
            );
            write(
                &path,
                &Patch {
                    artwork: Artwork::Replace(webp.clone()),
                    ..Default::default()
                },
            )
            .unwrap_or_else(|error| panic!("{extension} WebP: {error}"));
            assert_eq!(
                read(&path).unwrap().artwork,
                Some(webp.clone()),
                "{extension} WebP"
            );
            assert_eq!(
                decode(),
                before,
                "{extension} audio changed after WebP cover replacement"
            );
            write(
                &path,
                &Patch {
                    fields: if extension == "wma" {
                        BTreeMap::new()
                    } else {
                        BTreeMap::from([("MY_CUSTOM_TAG".into(), vec![])])
                    },
                    artwork: Artwork::Remove,
                },
            )
            .unwrap_or_else(|error| panic!("{extension} removal: {error}"));
            let metadata = read(&path).unwrap();
            assert!(
                metadata.artwork.is_none() && !metadata.fields.contains_key("MY_CUSTOM_TAG"),
                "{extension}"
            );
            assert_eq!(metadata.fields["TITLE"], ["New title"], "{extension}");
            assert_eq!(decode(), before, "{extension} audio changed after removal");
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn edits_multivalue_custom_tags_without_changing_audio_or_unedited_tags() {
        let dir = std::env::temp_dir().join(format!("feishin-tags-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.wav");
        let audio: Vec<u8> = (0..8000)
            .flat_map(|n| ((n % 100) as i16).to_le_bytes())
            .collect();
        let mut wave = Vec::from(&b"RIFF"[..]);
        wave.extend((36 + audio.len() as u32).to_le_bytes());
        wave.extend(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        wave.extend(8000u32.to_le_bytes());
        wave.extend(16000u32.to_le_bytes());
        wave.extend(b"\x02\0\x10\0data");
        wave.extend((audio.len() as u32).to_le_bytes());
        wave.extend(&audio);
        fs::write(&path, &wave).unwrap();
        let mut file = Probe::open(&path).unwrap().read().unwrap();
        let mut tag = Tag::new(TagType::Id3v2);
        tag.insert_text(ItemKey::AlbumTitle, "Keep album".into());
        file.insert_tag(tag);
        file.save_to_path(&path, WriteOptions::default()).unwrap();
        let patch = Patch {
            fields: BTreeMap::from([
                ("TITLE".into(), vec!["New title".into()]),
                ("ARTIST".into(), vec!["First".into(), "Second".into()]),
                ("MY_CUSTOM_TAG".into(), vec!["Custom value".into()]),
                ("ABCD".into(), vec!["A".into(), "B".into()]),
            ]),
            artwork: Artwork::Keep,
        };
        write(&path, &patch).unwrap();
        let metadata = read(&path).unwrap();
        assert_eq!(metadata.fields["TITLE"], ["New title"]);
        assert_eq!(metadata.fields["ALBUM"], ["Keep album"]);
        assert_eq!(metadata.fields["ARTIST"], ["First", "Second"]);
        assert_eq!(metadata.fields["MY_CUSTOM_TAG"], ["Custom value"]);
        let before = fs::read(&path).unwrap();
        assert!(before.windows(audio.len()).any(|data| data == audio));
        let invalid = Patch {
            fields: BTreeMap::from([("BAD=KEY".into(), vec!["fail".into()])]),
            ..Patch::default()
        };
        assert!(write(&path, &invalid).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
