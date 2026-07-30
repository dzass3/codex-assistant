use std::fs;

use codex_assistant_lib::{
    local_theme::LocalThemeCatalog,
    theme::{
        theme_application_source_with_asset, RightsStatus, ThemeAsset, ThemeBackdrop,
        ThemeCategory, ThemeEffects, ThemePack, ThemePalette, ThemeRights,
        MAX_RUNTIME_THEME_ASSET_BYTES,
    },
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

fn pack(hash: String) -> ThemePack {
    ThemePack {
        schema_version: 1,
        minimum_engine_version: 1,
        id: "arina-pink".into(),
        name: "Arina Pink".into(),
        description: "User-owned local theme".into(),
        category: ThemeCategory::LocalImport,
        marketplace: None,
        adaptation: None,
        preview_path: "local-theme:arina-pink".into(),
        backdrop: ThemeBackdrop::Image {
            asset_id: "arina-pink".into(),
            overlay: "#fff5f6".into(),
            focal_x: 72,
            focal_y: 45,
        },
        palette: ThemePalette {
            surface: "#fff8f8".into(),
            surface_strong: "#fffdfd".into(),
            text: "#3b292d".into(),
            accent: "#d9637e".into(),
            border: "#e7aeba".into(),
        },
        effects: ThemeEffects {
            surface_opacity: 78,
            blur_px: 10,
            contrast_percent: 96,
            motion: false,
        },
        assets: vec![ThemeAsset {
            id: "arina-pink".into(),
            mime_type: "image/jpeg".into(),
            sha256: hash,
        }],
        rights: ThemeRights {
            source: "User-owned local import".into(),
            rightsholder: "User-provided asset".into(),
            license: "Local use only".into(),
            commercial_redistribution: false,
            attribution: "Stored locally by user request".into(),
            reviewed_at: "2026-07-19".into(),
            manual_signoff: true,
            status: RightsStatus::LocalOnly,
        },
    }
}

fn write_pack(root: &std::path::Path, bytes: &[u8], hash: String) {
    let directory = root.join("local-themes").join("arina-pink");
    fs::create_dir_all(&directory).expect("theme directory");
    fs::write(
        directory.join("theme.json"),
        serde_json::to_vec_pretty(&pack(hash)).expect("manifest"),
    )
    .expect("write manifest");
    fs::write(directory.join("arina-pink.jpg"), bytes).expect("write asset");
}

#[test]
fn local_catalog_loads_one_hash_verified_image_pack() {
    let root = tempdir().expect("tempdir");
    let bytes = b"local-image";
    write_pack(root.path(), bytes, format!("{:x}", Sha256::digest(bytes)));

    let catalog = LocalThemeCatalog::in_directory(root.path()).expect("catalog");
    let packs = catalog.packs();

    assert_eq!(packs.len(), 1);
    assert_eq!(packs[0].category, ThemeCategory::LocalImport);
    assert_eq!(packs[0].rights.status, RightsStatus::LocalOnly);
    assert_eq!(
        catalog.asset_bytes("arina-pink").as_deref(),
        Some(bytes.as_slice())
    );
    assert!(catalog
        .preview_data_url("arina-pink")
        .expect("preview")
        .starts_with("data:image/jpeg;base64,"));
}

#[test]
fn local_catalog_fails_closed_for_a_hash_mismatch() {
    let root = tempdir().expect("tempdir");
    write_pack(root.path(), b"tampered", "0".repeat(64));

    let catalog = LocalThemeCatalog::in_directory(root.path()).expect("catalog");

    assert!(catalog.packs().is_empty());
    assert!(catalog.asset_bytes("arina-pink").is_none());
    assert!(catalog.preview_data_url("arina-pink").is_none());
}

#[test]
fn local_catalog_accepts_only_assets_that_fit_the_encoded_theme_budget() {
    let accepted_root = tempdir().expect("accepted tempdir");
    let accepted = vec![7_u8; MAX_RUNTIME_THEME_ASSET_BYTES as usize];
    write_pack(
        accepted_root.path(),
        &accepted,
        format!("{:x}", Sha256::digest(&accepted)),
    );
    let accepted_catalog =
        LocalThemeCatalog::in_directory(accepted_root.path()).expect("accepted catalog");
    let accepted_bytes = accepted_catalog
        .asset_bytes("arina-pink")
        .expect("accepted asset bytes");
    assert_eq!(accepted_bytes, accepted);
    let accepted_pack = accepted_catalog.packs().remove(0);
    assert!(theme_application_source_with_asset(&accepted_pack, Some(&accepted_bytes)).is_ok());

    let rejected_root = tempdir().expect("rejected tempdir");
    let rejected = vec![7_u8; MAX_RUNTIME_THEME_ASSET_BYTES as usize + 1];
    write_pack(
        rejected_root.path(),
        &rejected,
        format!("{:x}", Sha256::digest(&rejected)),
    );
    let rejected_catalog =
        LocalThemeCatalog::in_directory(rejected_root.path()).expect("rejected catalog");
    assert!(rejected_catalog.packs().is_empty());
    assert!(rejected_catalog.asset_bytes("arina-pink").is_none());
}

#[test]
fn local_catalog_imports_a_valid_image_as_a_device_only_theme() {
    let root = tempdir().expect("tempdir");
    let catalog = LocalThemeCatalog::in_directory(root.path()).expect("catalog");
    let bytes = include_bytes!("../../public/themes/wisteria-bride.webp");

    let imported = catalog
        .import_image("My Aurora", "image/webp", bytes)
        .expect("valid local import");

    assert!(imported.id.starts_with("local-"));
    assert_eq!(imported.name, "My Aurora");
    assert_eq!(imported.category, ThemeCategory::LocalImport);
    assert_eq!(imported.rights.status, RightsStatus::LocalOnly);
    assert!(!imported.rights.commercial_redistribution);
    assert_eq!(
        catalog.asset_bytes(&imported.id).as_deref(),
        Some(bytes.as_slice())
    );
    assert_eq!(catalog.packs(), vec![imported]);
}

#[test]
fn local_catalog_rejects_mime_spoofing_and_oversized_imports() {
    let root = tempdir().expect("tempdir");
    let catalog = LocalThemeCatalog::in_directory(root.path()).expect("catalog");
    let webp = include_bytes!("../../public/themes/wisteria-bride.webp");

    assert!(catalog.import_image("Spoof", "image/jpeg", webp).is_err());
    assert!(catalog
        .import_image(
            "Oversized",
            "image/webp",
            &vec![0_u8; MAX_RUNTIME_THEME_ASSET_BYTES as usize + 1],
        )
        .is_err());
    assert!(catalog.packs().is_empty());
}

#[test]
fn importing_identical_bytes_is_idempotent_and_sanitizes_the_display_name() {
    let root = tempdir().expect("tempdir");
    let catalog = LocalThemeCatalog::in_directory(root.path()).expect("catalog");
    let bytes = include_bytes!("../../public/themes/mint-gentleman.webp");

    let first = catalog
        .import_image("javascript:<script>", "image/webp", bytes)
        .expect("first import");
    let second = catalog
        .import_image("Different name", "image/webp", bytes)
        .expect("duplicate import");

    assert_eq!(first.id, second.id);
    assert_eq!(first.name, second.name);
    assert!(!first.name.contains("script"));
    assert_eq!(catalog.packs().len(), 1);
}
