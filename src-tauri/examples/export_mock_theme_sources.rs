use codex_assistant_lib::{
    local_theme::LocalThemeCatalog,
    theme::{
        bundled_theme_packs, theme_application_source, theme_application_source_with_asset,
        theme_page_classification_source, theme_restore_source, theme_verification_source,
    },
};
use serde::Serialize;

#[derive(Serialize)]
struct ThemeSource {
    id: String,
    application_source: String,
    verification_source: String,
}

#[derive(Serialize)]
struct ThemeSourceExport {
    themes: Vec<ThemeSource>,
    local_theme: ThemeSource,
    classification_source: String,
    restore_source: &'static str,
}

fn main() {
    let themes = bundled_theme_packs()
        .into_iter()
        .map(|pack| ThemeSource {
            id: pack.id.clone(),
            application_source: theme_application_source(&pack)
                .expect("bundled theme application source"),
            verification_source: theme_verification_source(&pack)
                .expect("bundled theme verification source"),
        })
        .collect();
    let local_root = tempfile::tempdir().expect("local theme tempdir");
    let catalog = LocalThemeCatalog::in_directory(local_root.path()).expect("local catalog");
    let local_bytes = include_bytes!("../../public/themes/wisteria-bride.webp");
    let local_pack = catalog
        .import_image("Mock Local Import", "image/webp", local_bytes)
        .expect("local import");
    let local_asset = catalog
        .asset_bytes(&local_pack.id)
        .expect("local import bytes");
    let local_theme = ThemeSource {
        id: local_pack.id.clone(),
        application_source: theme_application_source_with_asset(&local_pack, Some(&local_asset))
            .expect("local theme application source"),
        verification_source: theme_verification_source(&local_pack)
            .expect("local theme verification source"),
    };

    println!(
        "{}",
        serde_json::to_string(&ThemeSourceExport {
            themes,
            local_theme,
            classification_source: theme_page_classification_source(),
            restore_source: theme_restore_source(),
        })
        .expect("serialize theme sources")
    );
}
