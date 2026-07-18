use std::{fs, path::Path};

use serde_json::Value;
use sha2::{Digest, Sha256};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn bundled_theme_rights_manifest_matches_the_shipped_assets() {
    let manifest_path = root().join("resources/themes/rights.json");
    let manifest: Value = serde_json::from_slice(
        &fs::read(&manifest_path).expect("bundled themes need a rights manifest"),
    )
    .expect("theme rights manifest must be valid JSON");
    assert_eq!(manifest["schema_version"], 1);
    let entries = manifest["themes"].as_array().expect("theme entries");
    assert_eq!(entries.len(), 2);
    for entry in entries {
        assert_eq!(entry["rights"]["status"], "verified");
        assert_eq!(entry["rights"]["commercial_redistribution"], true);
        assert!(entry["rights"]["reviewed_at"].as_str().is_some());
    }

    let muse = entries
        .iter()
        .find(|entry| entry["id"] == "observatory-muse")
        .expect("original character rights entry");
    let asset = fs::read(root().join("resources/themes/original-observatory-muse.jpg"))
        .expect("original character bundled asset");
    assert_eq!(
        muse["assets"][0]["sha256"],
        format!("{:x}", Sha256::digest(asset))
    );
}

#[test]
fn frontend_previews_and_theme_rights_ship_in_the_desktop_bundle() {
    for preview in [
        "../public/themes/aurora-grid.svg",
        "../public/themes/original-observatory-muse.jpg",
    ] {
        assert!(root().join(preview).is_file(), "missing preview: {preview}");
    }
    let config = fs::read_to_string(root().join("tauri.conf.json")).expect("Tauri config");
    assert!(
        config.contains("resources/themes/**/*"),
        "theme rights and engine assets must be included in the desktop bundle"
    );
}

#[test]
fn upstream_reference_is_pinned_and_excluded_media_is_not_redistributed() {
    let notices =
        fs::read_to_string(root().join("../THIRD_PARTY_NOTICES.md")).expect("third-party notices");
    assert!(notices.contains("3af1d6d62f3a0388cc640d2f497ac3100998938e"));
    assert!(notices.contains("not copied or redistributed"));
    for forbidden in ["arina", "hashimoto", "celebrity", "franchise"] {
        let paths = [
            root().join("resources/themes"),
            root().join("../public/themes"),
        ];
        for directory in paths {
            for entry in fs::read_dir(directory).expect("theme directory") {
                let name = entry
                    .expect("theme file")
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                assert!(
                    !name.contains(forbidden),
                    "excluded media was bundled: {name}"
                );
            }
        }
    }
}
