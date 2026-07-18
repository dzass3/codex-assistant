use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::control_layer::cdp::{
    fetch_page_targets, BrowserEndpoint, CdpClient, CdpClientError, CdpDiscoveryError,
};

const ENGINE_VERSION: u32 = 1;
const OBSERVATORY_MUSE: &[u8] = include_bytes!("../resources/themes/original-observatory-muse.jpg");

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
}

pub fn bundled_theme_packs() -> Vec<ThemePack> {
    vec![abstract_aurora(), observatory_muse()]
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
        && valid_date(&pack.rights.reviewed_at);
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
            let bytes = asset_bytes(asset_id).ok_or(ThemeValidationError::InvalidAsset)?;
            let encoded = STANDARD.encode(bytes);
            format!(
                "linear-gradient({overlay}99, {overlay}99), url(\\\"data:image/jpeg;base64,{encoded}\\\") {focal_x}% {focal_y}% / cover no-repeat fixed"
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
    timeout_ms: u64,
) -> Result<usize, ThemeEngineError> {
    let source = theme_application_source(pack).map_err(ThemeEngineError::InvalidPack)?;
    let targets = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(ThemeEngineError::Discovery)?;
    let mut applied = 0;
    for target in targets {
        let mut client = CdpClient::connect_target(&target, endpoint.port(), timeout_ms)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        client
            .call("Page.enable", serde_json::json!({}))
            .await
            .map_err(ThemeEngineError::Cdp)?;
        client
            .call(
                "Page.addScriptToEvaluateOnNewDocument",
                serde_json::json!({"source": source}),
            )
            .await
            .map_err(ThemeEngineError::Cdp)?;
        if client
            .evaluate_boolean(&source)
            .await
            .map_err(ThemeEngineError::Cdp)?
        {
            applied += 1;
        }
    }
    Ok(applied)
}

pub async fn restore_theme_on_pages(
    endpoint: &BrowserEndpoint,
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

fn abstract_aurora() -> ThemePack {
    ThemePack {
        schema_version: 1,
        minimum_engine_version: ENGINE_VERSION,
        id: "aurora-grid".into(),
        name: "Aurora Grid".into(),
        description: "Project-owned abstract aurora with restrained glass surfaces.".into(),
        category: ThemeCategory::Abstract,
        preview_path: "/themes/aurora-grid.svg".into(),
        backdrop: ThemeBackdrop::Gradient {
            angle: 135,
            colors: ["#07111f".into(), "#18204b".into(), "#0b4d5f".into()],
        },
        palette: ThemePalette {
            surface: "#101827".into(),
            surface_strong: "#111b2d".into(),
            text: "#eef7ff".into(),
            accent: "#64e7ff".into(),
            border: "#6fdcf0".into(),
        },
        effects: ThemeEffects {
            surface_opacity: 78,
            blur_px: 22,
            contrast_percent: 108,
            motion: true,
        },
        assets: Vec::new(),
        rights: project_rights("Original abstract theme authored for Codex Assistant"),
    }
}

fn observatory_muse() -> ThemePack {
    let hash = format!("{:x}", Sha256::digest(OBSERVATORY_MUSE));
    ThemePack {
        schema_version: 1,
        minimum_engine_version: ENGINE_VERSION,
        id: "observatory-muse".into(),
        name: "Observatory Muse".into(),
        description: "Original fictional technologist in a quiet violet observatory.".into(),
        category: ThemeCategory::OriginalCharacter,
        preview_path: "/themes/original-observatory-muse.jpg".into(),
        backdrop: ThemeBackdrop::Image {
            asset_id: "original-observatory-muse".into(),
            overlay: "#071326".into(),
            focal_x: 50,
            focal_y: 50,
        },
        palette: ThemePalette {
            surface: "#0c1730".into(),
            surface_strong: "#101a35".into(),
            text: "#f4f2ff".into(),
            accent: "#a990ff".into(),
            border: "#7f8cff".into(),
        },
        effects: ThemeEffects {
            surface_opacity: 76,
            blur_px: 24,
            contrast_percent: 105,
            motion: false,
        },
        assets: vec![ThemeAsset {
            id: "original-observatory-muse".into(),
            mime_type: "image/jpeg".into(),
            sha256: hash,
        }],
        rights: project_rights(
            "Original fictional character generated for Codex Assistant on 2026-07-18",
        ),
    }
}

fn project_rights(source: &str) -> ThemeRights {
    ThemeRights {
        source: source.into(),
        rightsholder: "Codex Assistant project".into(),
        license: "Project-owned distribution asset".into(),
        commercial_redistribution: true,
        attribution: "Original artwork created for Codex Assistant".into(),
        reviewed_at: "2026-07-18".into(),
        status: RightsStatus::Verified,
    }
}

fn asset_bytes(asset_id: &str) -> Option<&'static [u8]> {
    match asset_id {
        "original-observatory-muse" => Some(OBSERVATORY_MUSE),
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
