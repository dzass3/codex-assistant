use std::{fs, path::Path};

use serde_json::Value;
use sha2::{Digest, Sha256};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn bundled_theme_rights_manifest_matches_the_shipped_assets() {
    let manifest_path = root().join("../shared/theme-catalog.json");
    let manifest: Value = serde_json::from_slice(
        &fs::read(&manifest_path).expect("bundled themes need a rights manifest"),
    )
    .expect("theme rights manifest must be valid JSON");
    assert_eq!(manifest["schema_version"], 1);
    let entries = manifest["themes"].as_array().expect("theme entries");
    assert_eq!(entries.len(), 14);
    for entry in entries {
        assert_eq!(entry["rights"]["status"], "verified");
        assert_eq!(entry["rights"]["commercial_redistribution"], true);
        assert_eq!(entry["rights"]["manual_signoff"], true);
        assert!(entry["rights"]["reviewed_at"].as_str().is_some());
        let preview = entry["preview_path"]
            .as_str()
            .expect("theme preview path")
            .trim_start_matches('/');
        assert!(
            root().join("../public").join(preview).is_file(),
            "missing preview for {}",
            entry["id"]
        );
    }
}

#[test]
fn wisteria_bride_catalog_entry_matches_the_reproducible_build_manifest() {
    let catalog: Value = serde_json::from_slice(
        &fs::read(root().join("../shared/theme-catalog.json")).expect("theme catalog"),
    )
    .expect("valid theme catalog");
    let generated_pack: Value = serde_json::from_slice(
        &fs::read(root().join("../shared/generated-theme-packs/wisteria-bride.json"))
            .expect("generated Wisteria Bride pack"),
    )
    .expect("valid generated pack");
    let build_manifest: Value = serde_json::from_slice(
        &fs::read(root().join("../assets/theme-sources/wisteria-bride/build-manifest.json"))
            .expect("Wisteria Bride build manifest"),
    )
    .expect("valid build manifest");
    let catalog_pack = catalog["themes"]
        .as_array()
        .expect("theme entries")
        .iter()
        .find(|entry| entry["id"] == "wisteria-bride")
        .expect("Wisteria Bride catalog entry");
    assert_eq!(catalog_pack, &generated_pack);

    let source = fs::read(root().join("../assets/theme-sources/wisteria-bride/source.png"))
        .expect("approved Wisteria Bride source");
    let runtime = fs::read(root().join("resources/themes/wisteria-bride.webp"))
        .expect("Wisteria Bride runtime WebP");
    let preview = fs::read(root().join("../public/themes/wisteria-bride.webp"))
        .expect("Wisteria Bride preview WebP");

    assert_eq!(
        build_manifest["source"]["sha256"],
        format!("{:x}", Sha256::digest(&source))
    );
    assert_eq!(
        build_manifest["runtime"]["sha256"],
        format!("{:x}", Sha256::digest(&runtime))
    );
    assert_eq!(
        build_manifest["preview"]["sha256"],
        format!("{:x}", Sha256::digest(&preview))
    );
    assert_eq!(
        generated_pack["assets"][0]["sha256"],
        build_manifest["runtime"]["sha256"]
    );
    assert_eq!(build_manifest["runtime"]["bytes"], runtime.len() as u64);
    assert_eq!(build_manifest["preview"]["bytes"], preview.len() as u64);
}

#[test]
fn approved_fourteen_source_files_match_their_generated_offline_artifacts() {
    let plan: Value = serde_json::from_slice(
        &fs::read(root().join("../assets/theme-sources/catalog-plan.json"))
            .expect("approved theme catalog plan"),
    )
    .expect("valid theme catalog plan");
    let catalog: Value = serde_json::from_slice(
        &fs::read(root().join("../shared/theme-catalog.json")).expect("theme catalog"),
    )
    .expect("valid theme catalog");
    let planned = plan["themes"].as_array().expect("planned themes");
    assert_eq!(planned.len(), 14);
    assert_eq!(
        catalog["themes"].as_array().expect("catalog themes").len(),
        14
    );

    for theme in planned {
        let id = theme["id"].as_str().expect("stable theme id");
        let source_file = theme["source_file"]
            .as_str()
            .expect("original source filename");
        let manifest: Value = serde_json::from_slice(
            &fs::read(
                root()
                    .join("../assets/theme-sources")
                    .join(id)
                    .join("build-manifest.json"),
            )
            .unwrap_or_else(|_| panic!("missing build manifest for {id}")),
        )
        .expect("valid build manifest");
        let generated_pack: Value = serde_json::from_slice(
            &fs::read(
                root()
                    .join("../shared/generated-theme-packs")
                    .join(format!("{id}.json")),
            )
            .unwrap_or_else(|_| panic!("missing generated pack for {id}")),
        )
        .expect("valid generated pack");
        let catalog_pack = catalog["themes"]
            .as_array()
            .expect("catalog entries")
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("missing catalog entry for {id}"));
        assert_eq!(catalog_pack, &generated_pack);
        assert_eq!(manifest["source"]["original_file_name"], source_file);
        assert_eq!(manifest["source"]["sha256"], theme["sha256"]);
        assert_eq!(manifest["source"]["bytes"], theme["bytes"]);
        assert_eq!(manifest["source"]["width"], theme["width"]);
        assert_eq!(manifest["source"]["height"], theme["height"]);
        let source_width = manifest["source"]["width"].as_u64().expect("source width");
        let source_height = manifest["source"]["height"]
            .as_u64()
            .expect("source height");
        for variant in ["runtime", "preview"] {
            let variant_width = manifest[variant]["width"]
                .as_u64()
                .unwrap_or_else(|| panic!("{variant} width for {id}"));
            let variant_height = manifest[variant]["height"]
                .as_u64()
                .unwrap_or_else(|| panic!("{variant} height for {id}"));
            let scaled_width = source_width * variant_height;
            let scaled_height = source_height * variant_width;
            assert!(
                scaled_width.abs_diff(scaled_height) <= source_width.max(source_height),
                "{id} {variant} must preserve the complete source aspect ratio within one rounded pixel, without cropping"
            );
        }

        let runtime = fs::read(root().join("resources/themes").join(format!("{id}.webp")))
            .unwrap_or_else(|_| panic!("missing runtime resource for {id}"));
        let preview = fs::read(root().join("../public/themes").join(format!("{id}.webp")))
            .unwrap_or_else(|_| panic!("missing preview resource for {id}"));
        assert_eq!(
            manifest["runtime"]["sha256"],
            format!("{:x}", Sha256::digest(&runtime))
        );
        assert_eq!(
            manifest["preview"]["sha256"],
            format!("{:x}", Sha256::digest(&preview))
        );
    }
}

#[test]
fn frontend_previews_and_theme_rights_ship_in_the_desktop_bundle() {
    let catalog: Value = serde_json::from_slice(
        &fs::read(root().join("../shared/theme-catalog.json")).expect("theme catalog"),
    )
    .expect("valid theme catalog");
    for entry in catalog["themes"].as_array().expect("theme entries") {
        let preview = entry["preview_path"]
            .as_str()
            .expect("preview path")
            .trim_start_matches('/');
        assert!(root().join("../public").join(preview).is_file());
        let id = entry["id"].as_str().expect("theme id");
        assert!(root()
            .join("resources/themes")
            .join(format!("{id}.webp"))
            .is_file());
    }
    for retired in [
        "../public/themes/aurora-grid.webp",
        "../public/themes/original-observatory-muse.jpg",
        "resources/themes/original-observatory-muse.jpg",
    ] {
        assert!(
            !root().join(retired).exists(),
            "retired asset remains: {retired}"
        );
    }
    let config = fs::read_to_string(root().join("tauri.conf.json")).expect("Tauri config");
    assert!(
        config.contains("resources/themes/**/*"),
        "theme rights and engine assets must be included in the desktop bundle"
    );
    assert!(
        config.contains("../shared/theme-catalog.json"),
        "the single theme catalog must ship in the desktop bundle"
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
