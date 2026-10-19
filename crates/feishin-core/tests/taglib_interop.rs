use base64::Engine;
use feishin_core::tags::{self, Artwork, Fields, Patch};
use std::{fs, path::Path, process::Command};

#[test]
#[ignore = "Requires FEISHIN_TAGLIB_REFERENCE pointing to archived metadata-service fixtures"]
fn original_metadata_service_files_match_native_taglib() {
    let directory = std::env::var("FEISHIN_TAGLIB_REFERENCE").expect("Original metadata fixtures");
    let reference: serde_json::Value =
        serde_json::from_slice(&fs::read(Path::new(&directory).join("reference.json")).unwrap())
            .unwrap();
    let artwork = base64::engine::general_purpose::STANDARD
        .decode(reference["artwork"].as_str().unwrap())
        .unwrap();
    let ffmpeg = std::env::var("FEISHIN_TEST_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    for case in reference["cases"].as_array().unwrap() {
        let extension = case["extension"].as_str().unwrap();
        let native = Path::new(&directory).join(format!("native.{extension}"));
        let baseline = Path::new(case["baseline"].as_str().unwrap());
        fs::copy(baseline, &native).unwrap();
        if extension == "wav" {
            // The original WASM API normalizes 03/12 to 3. Keep the native
            // preservation guarantee instead of silently losing the entered text.
            tags::write(
                &native,
                &Patch {
                    fields: [("TRACKNUMBER".into(), vec!["03/12".into()])].into(),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                tags::read(&native).unwrap().fields["TRACKNUMBER"],
                ["03/12"]
            );
        }
        let mut fields: Fields = serde_json::from_value(case["fields"].clone()).unwrap();
        if extension == "wma" {
            // ASF has no TRACKTOTAL property. Reject it without changing the file,
            // whereas the original WASM API reports success and drops the total.
            let before = fs::read(&native).unwrap();
            assert!(
                tags::write(
                    &native,
                    &Patch {
                        fields: [("TRACKTOTAL".into(), vec!["12".into()])].into(),
                        ..Default::default()
                    }
                )
                .is_err()
            );
            assert_eq!(fs::read(&native).unwrap(), before);
            fields.remove("TRACKTOTAL");
        }
        tags::write(
            &native,
            &Patch {
                fields,
                artwork: Artwork::Replace(artwork.clone()),
            },
        )
        .unwrap_or_else(|error| panic!("{extension}: {error}"));
        let compare = |path: &Path, expected: &serde_json::Value| {
            let actual = tags::read(path).unwrap();
            let fields: Fields = serde_json::from_value(expected["fields"].clone()).unwrap();
            assert_eq!(actual.fields, fields, "{extension}: {}", path.display());
            let artwork = expected["artwork"].as_str().map(|value| {
                base64::engine::general_purpose::STANDARD
                    .decode(value)
                    .unwrap()
            });
            assert_eq!(actual.artwork, artwork, "{extension}: cover artwork");
        };
        compare(
            Path::new(case["original"].as_str().unwrap()),
            &case["expected"],
        );
        // WASM's basic-tag synchronization drops TRACKTOTAL on these formats.
        // The native writer must retain the requested total, with all other
        // properties matching the original service's output.
        let with_preserved_total = |expected: &serde_json::Value| {
            let mut expected = expected.clone();
            if ["mp3", "m4a", "alac.m4a", "aac"].contains(&extension) {
                assert!(
                    expected["fields"].get("TRACKTOTAL").is_none(),
                    "Original total handling changed"
                );
                expected["fields"]["TRACKTOTAL"] = serde_json::json!(["12"]);
            }
            expected
        };
        compare(&native, &with_preserved_total(&case["expected"]));
        tags::write(
            &native,
            &Patch {
                fields: [("GENRE".into(), vec![]), ("LYRICS".into(), vec![])].into(),
                artwork: Artwork::Remove,
            },
        )
        .unwrap_or_else(|error| panic!("{extension}: removal: {error}"));
        compare(
            Path::new(case["cleared"].as_str().unwrap()),
            &case["cleared_expected"],
        );
        compare(&native, &with_preserved_total(&case["cleared_expected"]));
        let decode = |path: &Path| {
            let output = Command::new(&ffmpeg)
                .args(["-v", "error", "-i"])
                .arg(path)
                .args(["-f", "s16le", "-c:a", "pcm_s16le", "-"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{extension}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        };
        assert_eq!(
            decode(baseline),
            decode(&native),
            "{extension}: decoded audio changed"
        );
    }
}
