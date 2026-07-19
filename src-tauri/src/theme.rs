use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::control_layer::cdp::{
    fetch_page_targets, BrowserEndpoint, CdpClient, CdpClientError, CdpDiscoveryError,
};

const ENGINE_VERSION: u32 = 1;
const OBSERVATORY_MUSE: &[u8] = include_bytes!("../resources/themes/original-observatory-muse.jpg");
const GOTHIC_HORIZON: &[u8] = include_bytes!("../../public/themes/gothic-horizon.webp");
const ROSEGLASS_ATELIER: &[u8] = include_bytes!("../../public/themes/roseglass-atelier.webp");
const BLUSH_CIRCUIT: &[u8] = include_bytes!("../../public/themes/blush-circuit.webp");
const FORTUNE_FOUNDRY: &[u8] = include_bytes!("../../public/themes/fortune-foundry.webp");
const CRIMSON_RELAY: &[u8] = include_bytes!("../../public/themes/crimson-relay.webp");
const CRYSTAL_DAYLIGHT: &[u8] = include_bytes!("../../public/themes/crystal-daylight.webp");
const POCKET_COSMOS: &[u8] = include_bytes!("../../public/themes/pocket-cosmos.webp");
const VIOLET_AFTERDARK: &[u8] = include_bytes!("../../public/themes/violet-afterdark.webp");
const CYAN_CHORUS: &[u8] = include_bytes!("../../public/themes/cyan-chorus.webp");
const NOIR_STAGE: &[u8] = include_bytes!("../../public/themes/noir-stage.webp");
const THEME_CATALOG: &str = include_str!("../../shared/theme-catalog.json");

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeCategory {
    Abstract,
    OriginalCharacter,
    ProjectShowcase,
    LocalImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RightsStatus {
    Verified,
    LocalOnly,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ThemeBackdrop {
    Gradient {
        angle: u16,
        colors: [String; 3],
    },
    Image {
        asset_id: String,
        overlay: String,
        focal_x: u8,
        focal_y: u8,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemePalette {
    pub surface: String,
    pub surface_strong: String,
    pub text: String,
    pub accent: String,
    pub border: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemeEffects {
    pub surface_opacity: u8,
    pub blur_px: u8,
    pub contrast_percent: u8,
    pub motion: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemeAsset {
    pub id: String,
    pub mime_type: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemeRights {
    pub source: String,
    pub rightsholder: String,
    pub license: String,
    pub commercial_redistribution: bool,
    pub attribution: String,
    pub reviewed_at: String,
    pub manual_signoff: bool,
    pub status: RightsStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemePack {
    pub schema_version: u32,
    pub minimum_engine_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ThemeCategory,
    pub preview_path: String,
    pub backdrop: ThemeBackdrop,
    pub palette: ThemePalette,
    pub effects: ThemeEffects,
    pub assets: Vec<ThemeAsset>,
    pub rights: ThemeRights,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeValidationError {
    InvalidMetadata,
    InvalidAppearance,
    InvalidAsset,
    RightsGate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeEngineError {
    InvalidPack(ThemeValidationError),
    Discovery(CdpDiscoveryError),
    Cdp(CdpClientError),
    DomIncompatible,
    PartialApplication,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeScriptRegistration {
    pub target_id: String,
    pub identifier: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeApplyResult {
    pub applied_pages: usize,
    pub scripts: Vec<ThemeScriptRegistration>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeCatalog {
    schema_version: u32,
    themes: Vec<ThemePack>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ThemePreferenceRecord {
    schema_version: u32,
    selected_theme_id: Option<String>,
}

pub struct ThemePreferenceStore {
    directory: PathBuf,
    state_file: PathBuf,
}

impl ThemePreferenceStore {
    pub fn in_directory(directory: &Path) -> Result<Self, String> {
        if directory.exists()
            && directory
                .symlink_metadata()
                .map_err(|_| "Theme preference directory is unavailable")?
                .file_type()
                .is_symlink()
        {
            return Err("Theme preference directory is unavailable".to_owned());
        }
        fs::create_dir_all(directory)
            .map_err(|_| "Theme preference directory is unavailable".to_owned())?;
        crate::routing::state::protect_owned_path(directory)?;
        let state_file = directory.join("theme-state.json");
        if state_file.exists()
            && state_file
                .symlink_metadata()
                .map_err(|_| "Theme preference state is unavailable")?
                .file_type()
                .is_symlink()
        {
            return Err("Theme preference state is unavailable".to_owned());
        }
        Ok(Self {
            directory: directory.to_path_buf(),
            state_file,
        })
    }

    pub fn load(&self) -> Result<Option<String>, String> {
        let bytes = match fs::read(&self.state_file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Theme preference state is unavailable".to_owned()),
        };
        if bytes.len() > 512 {
            return Err("Theme preference state is invalid".to_owned());
        }
        let record: ThemePreferenceRecord = serde_json::from_slice(&bytes)
            .map_err(|_| "Theme preference state is invalid".to_owned())?;
        if record.schema_version != 1
            || record.selected_theme_id.as_ref().is_some_and(|theme_id| {
                !bundled_theme_packs()
                    .iter()
                    .any(|pack| pack.id == *theme_id)
            })
        {
            return Err("Theme preference state is invalid".to_owned());
        }
        Ok(record.selected_theme_id)
    }

    pub fn save(&self, selected_theme_id: Option<&str>) -> Result<(), String> {
        if selected_theme_id
            .is_some_and(|theme_id| !bundled_theme_packs().iter().any(|pack| pack.id == theme_id))
        {
            return Err("Theme preference is invalid".to_owned());
        }
        let bytes = serde_json::to_vec(&ThemePreferenceRecord {
            schema_version: 1,
            selected_theme_id: selected_theme_id.map(str::to_owned),
        })
        .map_err(|_| "Theme preference state is invalid".to_owned())?;
        let temporary = self
            .directory
            .join(format!(".theme-state-{}.tmp", Uuid::new_v4()));
        let write_result = (|| {
            let mut file =
                File::create(&temporary).map_err(|_| "Theme preference state is unavailable")?;
            crate::routing::state::protect_owned_path(&temporary)?;
            file.write_all(&bytes)
                .map_err(|_| "Theme preference state is unavailable".to_owned())?;
            file.sync_all()
                .map_err(|_| "Theme preference state is unavailable".to_owned())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if crate::routing::state::replace_existing(&temporary, &self.state_file).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("Theme preference state is unavailable".to_owned());
        }
        Ok(())
    }
}

pub fn bundled_theme_packs() -> Vec<ThemePack> {
    let Ok(catalog) = serde_json::from_str::<ThemeCatalog>(THEME_CATALOG) else {
        return Vec::new();
    };
    if catalog.schema_version != 1
        || catalog
            .themes
            .iter()
            .any(|pack| validate_theme_pack(pack, true).is_err())
    {
        return Vec::new();
    }
    catalog.themes
}

pub fn validate_theme_pack(pack: &ThemePack, bundled: bool) -> Result<(), ThemeValidationError> {
    if pack.schema_version != 1
        || pack.minimum_engine_version == 0
        || pack.minimum_engine_version > ENGINE_VERSION
        || !safe_slug(&pack.id)
        || !safe_text(&pack.name, 80)
        || !safe_text(&pack.description, 240)
        || !safe_text(&pack.preview_path, 160)
        || !pack.preview_path.starts_with("/themes/")
    {
        return Err(ThemeValidationError::InvalidMetadata);
    }
    if !valid_hex(&pack.palette.surface)
        || !valid_hex(&pack.palette.surface_strong)
        || !valid_hex(&pack.palette.text)
        || !valid_hex(&pack.palette.accent)
        || !valid_hex(&pack.palette.border)
        || !(25..=100).contains(&pack.effects.surface_opacity)
        || pack.effects.blur_px > 40
        || !(80..=140).contains(&pack.effects.contrast_percent)
    {
        return Err(ThemeValidationError::InvalidAppearance);
    }
    match &pack.backdrop {
        ThemeBackdrop::Gradient { angle, colors } => {
            if *angle > 360 || !colors.iter().all(|color| valid_hex(color)) {
                return Err(ThemeValidationError::InvalidAppearance);
            }
        }
        ThemeBackdrop::Image {
            asset_id,
            overlay,
            focal_x,
            focal_y,
        } => {
            if !safe_slug(asset_id)
                || !valid_hex(overlay)
                || *focal_x > 100
                || *focal_y > 100
                || !pack.assets.iter().any(|asset| asset.id == *asset_id)
            {
                return Err(ThemeValidationError::InvalidAppearance);
            }
        }
    }
    if pack.assets.iter().any(|asset| {
        !safe_slug(&asset.id)
            || !matches!(
                asset.mime_type.as_str(),
                "image/jpeg" | "image/png" | "image/webp"
            )
            || asset.sha256.len() != 64
            || !asset
                .sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
    }) {
        return Err(ThemeValidationError::InvalidAsset);
    }
    let rights_valid = safe_text(&pack.rights.source, 240)
        && safe_text(&pack.rights.rightsholder, 120)
        && safe_text(&pack.rights.license, 120)
        && safe_text(&pack.rights.attribution, 240)
        && valid_date(&pack.rights.reviewed_at)
        && pack.rights.manual_signoff;
    if !rights_valid
        || (bundled
            && (pack.rights.status != RightsStatus::Verified
                || !pack.rights.commercial_redistribution))
    {
        return Err(ThemeValidationError::RightsGate);
    }
    Ok(())
}

pub fn theme_application_source(pack: &ThemePack) -> Result<String, ThemeValidationError> {
    validate_theme_pack(pack, false)?;
    let background = match &pack.backdrop {
        ThemeBackdrop::Gradient { angle, colors } => format!(
            "linear-gradient({angle}deg, {}, {}, {})",
            colors[0], colors[1], colors[2]
        ),
        ThemeBackdrop::Image {
            asset_id,
            overlay,
            focal_x,
            focal_y,
        } => {
            let asset = pack
                .assets
                .iter()
                .find(|asset| asset.id == *asset_id)
                .ok_or(ThemeValidationError::InvalidAsset)?;
            let bytes = asset_bytes(asset_id).ok_or(ThemeValidationError::InvalidAsset)?;
            let encoded = STANDARD.encode(bytes);
            format!(
                "linear-gradient({overlay}99, {overlay}99), url(\\\"data:{mime};base64,{encoded}\\\") {focal_x}% {focal_y}% / cover no-repeat fixed",
                mime = asset.mime_type,
            )
        }
    };
    let css = format!(
        r#"html,body,#root{{background:transparent!important}}body::before{{content:"";position:fixed;inset:0;z-index:-1;background:{background};filter:contrast({contrast}%);pointer-events:none}}main.main-surface,aside.app-shell-left-panel{{background:color-mix(in srgb,{surface} {opacity_percent}%,transparent)!important;backdrop-filter:blur({blur}px)}}[data-codex-composer="true"]{{background:{surface_strong}!important;border:1px solid {border}!important;box-shadow:0 18px 50px #00000033}}main.main-surface,aside.app-shell-left-panel,[data-codex-composer="true"]{{color:{text}!important}}button:focus-visible,a:focus-visible{{outline:2px solid {accent}!important;outline-offset:2px}}@media(prefers-reduced-motion:reduce){{*,*::before,*::after{{animation-duration:0.01ms!important;animation-iteration-count:1!important;transition-duration:0.01ms!important}}}}"#,
        contrast = pack.effects.contrast_percent,
        surface = pack.palette.surface,
        surface_strong = pack.palette.surface_strong,
        opacity_percent = pack.effects.surface_opacity,
        blur = pack.effects.blur_px,
        border = pack.palette.border,
        text = pack.palette.text,
        accent = pack.palette.accent,
    );
    let theme_id =
        serde_json::to_string(&pack.id).map_err(|_| ThemeValidationError::InvalidMetadata)?;
    let css = serde_json::to_string(&css).map_err(|_| ThemeValidationError::InvalidAppearance)?;
    let source = format!(
        r#"(()=>{{"use strict";const NAME="__codexAssistantThemeV1";const old=globalThis[NAME];if(old&&typeof old.destroy==="function")old.destroy();if(!document.querySelector("main.main-surface")||!document.querySelector("aside.app-shell-left-panel"))return false;const style=document.createElement("style");style.setAttribute("data-codex-assistant-theme",{theme_id});style.replaceChildren(document.createTextNode({css}));document.documentElement.append(style);const api=Object.freeze({{id:{theme_id},destroy(){{style.remove();if(globalThis[NAME]===api)delete globalThis[NAME]}}}});globalThis[NAME]=api;matchMedia("(prefers-reduced-motion: reduce)");return true}})()"#
    );
    if source.len() > 262_144 {
        return Err(ThemeValidationError::InvalidAsset);
    }
    Ok(source)
}

pub async fn apply_theme_on_pages(
    endpoint: &BrowserEndpoint,
    pack: &ThemePack,
    previous_scripts: &[ThemeScriptRegistration],
    timeout_ms: u64,
) -> Result<ThemeApplyResult, ThemeEngineError> {
    let source = theme_application_source(pack).map_err(ThemeEngineError::InvalidPack)?;
    let verification = theme_verification_source(pack).map_err(ThemeEngineError::InvalidPack)?;
    let targets = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(ThemeEngineError::Discovery)?;
    let mut compatible_targets = Vec::new();
    for target in &targets {
        let mut client = CdpClient::connect_target(target, endpoint.port(), timeout_ms)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        if client
            .evaluate_boolean(
                r#"(()=>Boolean(document.querySelector("main.main-surface")&&document.querySelector("aside.app-shell-left-panel")))()"#,
            )
            .await
            .map_err(ThemeEngineError::Cdp)?
        {
            compatible_targets.push(target.clone());
        }
    }
    if compatible_targets.is_empty() {
        return Err(ThemeEngineError::DomIncompatible);
    }
    let mut applied = 0;
    let mut scripts = Vec::new();
    for target in compatible_targets {
        let mut client = CdpClient::connect_target(&target, endpoint.port(), timeout_ms)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        client
            .call("Page.enable", serde_json::json!({}))
            .await
            .map_err(ThemeEngineError::Cdp)?;
        if let Some(previous) = previous_scripts
            .iter()
            .find(|script| script.target_id == target.target_id)
        {
            client
                .call(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    serde_json::json!({"identifier": previous.identifier}),
                )
                .await
                .map_err(ThemeEngineError::Cdp)?;
        }
        let identifier = client
            .register_script(&source)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        let inserted = client
            .evaluate_boolean(&source)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        let visible = inserted
            && client
                .evaluate_boolean(&verification)
                .await
                .map_err(ThemeEngineError::Cdp)?;
        if visible {
            applied += 1;
            scripts.push(ThemeScriptRegistration {
                target_id: target.target_id,
                identifier,
            });
        } else {
            client
                .call(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    serde_json::json!({"identifier": identifier}),
                )
                .await
                .map_err(ThemeEngineError::Cdp)?;
            client
                .evaluate_boolean(theme_restore_source())
                .await
                .map_err(ThemeEngineError::Cdp)?;
            return Err(ThemeEngineError::PartialApplication);
        }
    }
    Ok(ThemeApplyResult {
        applied_pages: applied,
        scripts,
    })
}

pub async fn restore_theme_on_pages(
    endpoint: &BrowserEndpoint,
    scripts: &[ThemeScriptRegistration],
    timeout_ms: u64,
) -> Result<usize, ThemeEngineError> {
    let targets = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(ThemeEngineError::Discovery)?;
    let mut restored = 0;
    for target in targets {
        let mut client = CdpClient::connect_target(&target, endpoint.port(), timeout_ms)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        if let Some(script) = scripts
            .iter()
            .find(|script| script.target_id == target.target_id)
        {
            client
                .call(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    serde_json::json!({"identifier": script.identifier}),
                )
                .await
                .map_err(ThemeEngineError::Cdp)?;
        }
        if client
            .evaluate_boolean(theme_restore_source())
            .await
            .map_err(ThemeEngineError::Cdp)?
        {
            restored += 1;
        }
    }
    Ok(restored)
}

pub fn theme_restore_source() -> &'static str {
    r#"(()=>{"use strict";const NAME="__codexAssistantThemeV1";const current=globalThis[NAME];if(current&&typeof current.destroy==="function")current.destroy();const owned=document.querySelector("style[data-codex-assistant-theme]");if(owned)owned.remove();return true})()"#
}

pub fn theme_verification_source(pack: &ThemePack) -> Result<String, ThemeValidationError> {
    validate_theme_pack(pack, false)?;
    let theme_id =
        serde_json::to_string(&pack.id).map_err(|_| ThemeValidationError::InvalidMetadata)?;
    Ok(format!(
        r#"(()=>{{"use strict";const id={theme_id};const api=globalThis.__codexAssistantThemeV1;const style=document.querySelector(`style[data-codex-assistant-theme="${{id}}"]`);const backdrop=getComputedStyle(document.body,"::before");return Boolean(api&&api.id===id&&style&&style.isConnected&&style.sheet&&style.sheet.cssRules.length>0&&backdrop.position==="fixed"&&backdrop.backgroundImage!=="none")}})()"#
    ))
}

fn asset_bytes(asset_id: &str) -> Option<&'static [u8]> {
    match asset_id {
        "original-observatory-muse" => Some(OBSERVATORY_MUSE),
        "gothic-horizon" => Some(GOTHIC_HORIZON),
        "roseglass-atelier" => Some(ROSEGLASS_ATELIER),
        "blush-circuit" => Some(BLUSH_CIRCUIT),
        "fortune-foundry" => Some(FORTUNE_FOUNDRY),
        "crimson-relay" => Some(CRIMSON_RELAY),
        "crystal-daylight" => Some(CRYSTAL_DAYLIGHT),
        "pocket-cosmos" => Some(POCKET_COSMOS),
        "violet-afterdark" => Some(VIOLET_AFTERDARK),
        "cyan-chorus" => Some(CYAN_CHORUS),
        "noir-stage" => Some(NOIR_STAGE),
        _ => None,
    }
}

fn safe_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn safe_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && !value.chars().any(char::is_control)
        && !value.to_ascii_lowercase().contains("javascript:")
        && !value.contains("<script")
        && !value.contains("http://")
        && !value.contains("https://")
}

fn valid_hex(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn valid_date(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .chars()
            .enumerate()
            .all(|(index, character)| matches!(index, 4 | 7) || character.is_ascii_digit())
}
