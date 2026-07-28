use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::control_layer::cdp::{
    fetch_page_targets, BrowserEndpoint, CdpClient, CdpClientError, CdpDiscoveryError,
};

const ENGINE_VERSION: u32 = 1;
const MAX_THEME_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_RUNTIME_THEME_ASSET_BYTES: u64 = 1_450_000;
const WISTERIA_BRIDE: &[u8] = include_bytes!("../resources/themes/wisteria-bride.webp");
const MINT_GENTLEMAN: &[u8] = include_bytes!("../resources/themes/mint-gentleman.webp");
const IRIS_GENTLEMAN: &[u8] = include_bytes!("../resources/themes/iris-gentleman.webp");
const CRIMSON_PALACE: &[u8] = include_bytes!("../resources/themes/crimson-palace.webp");
const VERDANT_FAIRY: &[u8] = include_bytes!("../resources/themes/verdant-fairy.webp");
const DESERT_PRINCE: &[u8] = include_bytes!("../resources/themes/desert-prince.webp");
const OASIS_PRINCE: &[u8] = include_bytes!("../resources/themes/oasis-prince.webp");
const SAKURA_MOON: &[u8] = include_bytes!("../resources/themes/sakura-moon.webp");
const SEASIDE_BLUE: &[u8] = include_bytes!("../resources/themes/seaside-blue.webp");
const AUTUMN_WUXIA: &[u8] = include_bytes!("../resources/themes/autumn-wuxia.webp");
const METEOR_EVENING: &[u8] = include_bytes!("../resources/themes/meteor-evening.webp");
const VIOLET_BLADE: &[u8] = include_bytes!("../resources/themes/violet-blade.webp");
const FUJI_AUTUMN: &[u8] = include_bytes!("../resources/themes/fuji-autumn.webp");
const SPRING_STREET: &[u8] = include_bytes!("../resources/themes/spring-street.webp");
const THEME_CATALOG: &str = include_str!("../../shared/theme-catalog.json");
const PAGE_CLASSIFIER: &str = include_str!("../resources/themes/page-adapter.js");
const THEME_ENHANCER: &str = include_str!("../resources/themes/theme-enhancer.js");
const SUPPORTED_CODEX_MAJOR: u32 = 26;
const SUPPORTED_CODEX_MINOR_MIN: u32 = 715;
const SUPPORTED_CODEX_MINOR_MAX: u32 = 799;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeAdapterId {
    OfficialStoreV26,
}

pub fn select_theme_adapter(version: &str) -> Option<ThemeAdapterId> {
    let parts = version
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.len() != 4
        || parts[0] != SUPPORTED_CODEX_MAJOR
        || !(SUPPORTED_CODEX_MINOR_MIN..=SUPPORTED_CODEX_MINOR_MAX).contains(&parts[1])
    {
        return None;
    }
    Some(ThemeAdapterId::OfficialStoreV26)
}

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
    UnsupportedVersion,
    AmbiguousPrimaryTarget,
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

pub(crate) struct ThemePreferenceLoad {
    pub selected_theme_id: Option<String>,
    pub removed_missing_bundled_theme: bool,
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
        crate::private_state::protect_owned_path(directory)?;
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

    pub(crate) fn load(&self, available: &[ThemePack]) -> Result<ThemePreferenceLoad, String> {
        let bytes = match fs::read(&self.state_file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ThemePreferenceLoad {
                    selected_theme_id: None,
                    removed_missing_bundled_theme: false,
                })
            }
            Err(_) => return Err("Theme preference state is unavailable".to_owned()),
        };
        if bytes.len() > 512 {
            return Err("Theme preference state is invalid".to_owned());
        }
        let record: ThemePreferenceRecord = serde_json::from_slice(&bytes)
            .map_err(|_| "Theme preference state is invalid".to_owned())?;
        if record.schema_version != 1 {
            return Err("Theme preference state is invalid".to_owned());
        }
        if record
            .selected_theme_id
            .as_ref()
            .is_some_and(|theme_id| !available.iter().any(|pack| pack.id == *theme_id))
        {
            self.save(None, available)?;
            return Ok(ThemePreferenceLoad {
                selected_theme_id: None,
                removed_missing_bundled_theme: true,
            });
        }
        Ok(ThemePreferenceLoad {
            selected_theme_id: record.selected_theme_id,
            removed_missing_bundled_theme: false,
        })
    }

    pub fn save(
        &self,
        selected_theme_id: Option<&str>,
        available: &[ThemePack],
    ) -> Result<(), String> {
        if selected_theme_id
            .is_some_and(|theme_id| !available.iter().any(|pack| pack.id == theme_id))
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
            crate::private_state::protect_owned_path(&temporary)?;
            file.write_all(&bytes)
                .map_err(|_| "Theme preference state is unavailable".to_owned())?;
            file.sync_all()
                .map_err(|_| "Theme preference state is unavailable".to_owned())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if crate::private_state::replace_existing(&temporary, &self.state_file).is_err() {
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
    let preview_valid = if pack.category == ThemeCategory::LocalImport && !bundled {
        pack.preview_path == format!("local-theme:{}", pack.id)
    } else {
        pack.preview_path.starts_with("/themes/")
    };
    if pack.schema_version != 1
        || pack.minimum_engine_version == 0
        || pack.minimum_engine_version > ENGINE_VERSION
        || !safe_slug(&pack.id)
        || !safe_text(&pack.name, 80)
        || !safe_text(&pack.description, 240)
        || !safe_text(&pack.preview_path, 160)
        || !preview_valid
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
    let image_bytes = match &pack.backdrop {
        ThemeBackdrop::Gradient { .. } => None,
        ThemeBackdrop::Image { asset_id, .. } => asset_bytes(asset_id),
    };
    theme_application_source_with_asset(pack, image_bytes)
}

pub fn theme_page_classification_source() -> String {
    format!("({PAGE_CLASSIFIER})()")
}

pub fn theme_application_source_with_asset(
    pack: &ThemePack,
    image_bytes: Option<&[u8]>,
) -> Result<String, ThemeValidationError> {
    validate_theme_pack(pack, false)?;
    let (background, background_size, background_repeat, background_position, focal_x, focal_y) =
        match &pack.backdrop {
            ThemeBackdrop::Gradient { angle, colors } => (
                format!(
                    "linear-gradient({angle}deg, {}, {}, {})",
                    colors[0], colors[1], colors[2]
                ),
                "cover".to_owned(),
                "no-repeat".to_owned(),
                "var(--codex-assistant-theme-focal-x) var(--codex-assistant-theme-focal-y)"
                    .to_owned(),
                74,
                42,
            ),
            ThemeBackdrop::Image {
                asset_id,
                overlay: _,
                focal_x,
                focal_y,
            } => {
                let asset = pack
                    .assets
                    .iter()
                    .find(|asset| asset.id == *asset_id)
                    .ok_or(ThemeValidationError::InvalidAsset)?;
                let bytes = image_bytes.ok_or(ThemeValidationError::InvalidAsset)?;
                if sha256_hex(bytes) != asset.sha256.to_ascii_lowercase() {
                    return Err(ThemeValidationError::InvalidAsset);
                }
                let encoded = STANDARD.encode(bytes);
                let image = format!("url(\"data:{};base64,{encoded}\")", asset.mime_type);
                (
                    image,
                    "cover".to_owned(),
                    "no-repeat".to_owned(),
                    "var(--codex-assistant-theme-focal-x) var(--codex-assistant-theme-focal-y)"
                        .to_owned(),
                    (*focal_x).max(74),
                    (*focal_y).min(42),
                )
            }
        };
    let narrow_focal_x = focal_x.clamp(79, 92);
    let ultrawide_focal_x = focal_x.min(70);
    let ultrawide_focal_y = focal_y.saturating_sub(2).max(32);
    let css = format!(
        r#":root{{--codex-assistant-theme-surface:{surface};--codex-assistant-theme-surface-strong:{surface_strong};--codex-assistant-theme-accent:{accent};--codex-assistant-theme-border:{border};--codex-assistant-theme-contrast:{contrast}%;--codex-assistant-theme-rose:#C67D91;--codex-assistant-theme-chrome:rgba(31,21,28,0.46);--codex-assistant-theme-chrome-strong:rgba(35,23,31,0.58);--codex-assistant-theme-chrome-text:rgba(255,248,251,0.94);--codex-assistant-theme-chrome-muted:rgba(255,238,244,0.68);--codex-assistant-theme-reading:rgba(255,250,252,0.76);--codex-assistant-theme-reading-text:#302A2D;--codex-assistant-theme-line:rgba(255,255,255,0.14);--codex-assistant-theme-shadow:rgba(10,6,9,0.18);--codex-assistant-theme-focal-x:{focal_x}%;--codex-assistant-theme-focal-y:{focal_y}%}}
html:root{{background-color:#151016;scrollbar-color:rgba(230,191,203,0.30) transparent;scrollbar-width:thin}}
body{{min-height:100vh;position:relative;isolation:isolate}}
body,#root{{background:transparent!important}}
body::before{{content:"";position:fixed;inset:0;background-color:#151016;background-image:{background};background-size:{background_size};background-repeat:{background_repeat};background-position:{background_position};background-attachment:fixed;filter:brightness(0.92) saturate(1.08) contrast(1.04);opacity:1;pointer-events:none;z-index:-2}}
body::after{{content:"";position:fixed;inset:0;background:linear-gradient(90deg,rgba(18,12,17,0.18) 0%,rgba(18,12,17,0.06) 42%,rgba(18,12,17,0.02) 72%,rgba(18,12,17,0.12) 100%);pointer-events:none;z-index:-1}}
::selection{{background:rgba(214,135,155,0.34)}}
body #root>div,body main.main-surface,body main[role="main"],body [data-codex-main="true"],body section[role="main"],body .app-shell-main-content-viewport,body [class*="main-layout"],body [class*="main-content"],body [class*="chat-page"],body [class*="chat-scroll-container"],body [class*="conversation-container"]{{background:transparent!important}}
body main.main-surface,body main[role="main"],body [data-codex-main="true"],body section[role="main"]{{position:relative}}
body aside.app-shell-left-panel,body aside[aria-label]:not([data-codex-output-panel]):not([aria-label*="output" i]):not([aria-label*="输出" i]),body nav[aria-label],body [data-codex-sidebar="true"]{{color:var(--codex-assistant-theme-chrome-text)!important;background:rgba(31,21,28,0.46)!important;backdrop-filter:blur(18px) saturate(130%)!important;border-right:1px solid rgba(255,255,255,0.12)!important;box-shadow:12px 0 30px rgba(10,6,9,0.16)!important;scrollbar-width:thin;scrollbar-color:rgba(230,191,203,0.28) transparent}}
body aside.app-shell-left-panel :where(h1,h2,h3,h4,p,span,a,button,[role="button"],label),body [data-codex-sidebar="true"] :where(h1,h2,h3,h4,p,span,a,button,[role="button"],label){{color:var(--codex-assistant-theme-chrome-text)!important}}
body aside.app-shell-left-panel svg,body [data-codex-sidebar="true"] svg{{color:var(--codex-assistant-theme-chrome-text)!important}}
body aside.app-shell-left-panel::-webkit-scrollbar,body [data-codex-sidebar="true"]::-webkit-scrollbar,body [data-codex-output-panel]::-webkit-scrollbar{{width:6px;height:6px}}
body aside.app-shell-left-panel::-webkit-scrollbar-thumb,body [data-codex-sidebar="true"]::-webkit-scrollbar-thumb,body [data-codex-output-panel]::-webkit-scrollbar-thumb{{background:rgba(230,191,203,0.28);border-radius:999px}}
body aside.app-shell-left-panel button,body aside.app-shell-left-panel [role="button"],body [data-codex-sidebar="true"] button,body [data-codex-sidebar="true"] [role="button"]{{transition:background-color 160ms ease,box-shadow 160ms ease}}
body aside.app-shell-left-panel button:hover,body aside.app-shell-left-panel [role="button"]:hover,body [data-codex-sidebar="true"] button:hover,body [data-codex-sidebar="true"] [role="button"]:hover{{background-color:rgba(255,255,255,0.09)!important}}
body aside.app-shell-left-panel [aria-current="page"],body [data-codex-sidebar="true"] [aria-current="page"]{{background:linear-gradient(90deg,rgba(214,135,155,0.28),rgba(214,135,155,0.10))!important;box-shadow:inset 3px 0 0 #C67D91!important}}
body .app-header-tint,body header[role="banner"],body [data-codex-header="true"]{{color:var(--codex-assistant-theme-chrome-text)!important;background:rgba(31,21,28,0.46)!important;backdrop-filter:blur(18px) saturate(130%)!important;border-bottom:1px solid rgba(255,255,255,0.10)!important;box-shadow:0 8px 24px rgba(10,6,9,0.12)!important}}
body .app-header-tint :where(strong,span,p,button,[role="button"]),body header[role="banner"] :where(strong,span,p,button,[role="button"]),body [data-codex-header="true"] :where(strong,span,p,button,[role="button"]){{color:var(--codex-assistant-theme-chrome-text)!important}}
body .app-header-tint button,body header[role="banner"] button,body [data-codex-header="true"] button{{transition:background-color 160ms ease}}
body .app-header-tint button:hover,body header[role="banner"] button:hover,body [data-codex-header="true"] button:hover{{background-color:rgba(255,255,255,0.09)!important}}
body [data-codex-output-panel],body [data-testid*="output-panel"],body aside[aria-label*="output" i],body aside[aria-label*="输出" i],body [class*="right-panel"],body [class*="artifact-panel"],body [class*="origin-top-right"][class*="pointer-events-none"]>[class*="pointer-events-auto"]>[class*="bg-token-dropdown-background"]{{color:var(--codex-assistant-theme-chrome-text)!important;background:rgba(35,23,31,0.58)!important;backdrop-filter:blur(20px) saturate(135%)!important;border:1px solid rgba(255,255,255,0.14)!important;border-radius:14px!important;box-shadow:0 14px 36px rgba(10,6,9,0.18)!important;scrollbar-width:thin;scrollbar-color:rgba(230,191,203,0.28) transparent}}
body [data-codex-output-panel] :where(strong,span,p,a,button,[role="button"]),body [data-testid*="output-panel"] :where(strong,span,p,a,button,[role="button"]),body [class*="right-panel"] :where(strong,span,p,a,button,[role="button"]),body [class*="artifact-panel"] :where(strong,span,p,a,button,[role="button"]),body [class*="origin-top-right"][class*="pointer-events-none"]>[class*="pointer-events-auto"]>[class*="bg-token-dropdown-background"] :where(strong,span,p,a,button,[role="button"]){{color:var(--codex-assistant-theme-chrome-text)!important}}
body [data-codex-output-panel] button,body [data-testid*="output-panel"] button,body [class*="right-panel"] button,body [class*="artifact-panel"] button,body [class*="origin-top-right"][class*="pointer-events-none"]>[class*="pointer-events-auto"]>[class*="bg-token-dropdown-background"] button{{transition:background-color 180ms ease}}
body [data-codex-output-panel] button:hover,body [data-testid*="output-panel"] button:hover,body [class*="right-panel"] button:hover,body [class*="artifact-panel"] button:hover,body [class*="origin-top-right"][class*="pointer-events-none"]>[class*="pointer-events-auto"]>[class*="bg-token-dropdown-background"] button:hover{{background-color:rgba(255,255,255,0.07)!important}}
body main .composer-surface-chrome,body main form[aria-label*="message" i],body main form[data-codex-composer="true"]{{color:var(--codex-assistant-theme-chrome-text)!important;background:rgba(29,22,28,0.72)!important;backdrop-filter:blur(22px) saturate(135%)!important;border:1px solid rgba(255,255,255,0.14)!important;border-radius:16px!important;box-shadow:0 18px 42px rgba(10,6,9,0.22),inset 0 1px 0 rgba(255,255,255,0.08)!important}}
body main .composer-surface-chrome :where(textarea,input,[contenteditable="true"]),body main form[aria-label*="message" i] :where(textarea,input,[contenteditable="true"]),body main form[data-codex-composer="true"] :where(textarea,input,[contenteditable="true"]){{color:var(--codex-assistant-theme-chrome-text)!important;caret-color:#E8A7B8!important;background:transparent!important}}
body main .composer-surface-chrome :where(textarea,input)::placeholder,body main form[aria-label*="message" i] :where(textarea,input)::placeholder,body main form[data-codex-composer="true"] :where(textarea,input)::placeholder{{color:var(--codex-assistant-theme-chrome-muted)!important;opacity:1}}
body main .composer-surface-chrome:focus-within,body main form[aria-label*="message" i]:focus-within,body main form[data-codex-composer="true"]:focus-within{{border-color:rgba(198,125,145,0.58)!important;box-shadow:0 20px 46px rgba(10,6,9,0.24),0 0 0 2px rgba(198,125,145,0.20)!important}}
body main.main-surface [class*="from-token-main-surface-primary"][class*="via-token-main-surface-primary"]{{background-image:linear-gradient(to top,rgba(18,12,17,0.20) 0%,rgba(18,12,17,0.06) 50%,rgba(18,12,17,0) 100%)!important}}
body main.main-surface [class*="bg-token-main-surface-primary"]{{background-color:transparent!important}}
body main.main-surface [class*="bg-token-button-background-secondary"]{{transition:background-color 160ms ease}}
body main.main-surface [class*="bg-token-button-background-secondary"]:hover{{background-color:rgba(255,255,255,0.12)!important}}
body main.main-surface [class*="bg-token-button-background-secondary"]:active{{background-color:rgba(214,135,155,0.18)!important}}
body main.main-surface [class*="bg-token-dropdown-background"]{{background:rgba(255,250,252,0.76)!important;border:1px solid rgba(255,255,255,0.25)!important;border-radius:14px!important;box-shadow:0 14px 32px rgba(10,6,9,0.12)!important;backdrop-filter:blur(16px) saturate(120%)}}
body main.main-surface [data-user-message-bubble="true"],body main.main-surface [data-message-author-role="user"],body main.main-surface [data-message-role="user"],body main.main-surface [data-testid*="user-message"],body main.main-surface [data-codex-reading-surface="true"],body main.main-surface [data-testid*="tool"],body main.main-surface [data-testid*="file"],body main.main-surface pre{{color:var(--codex-assistant-theme-reading-text)!important;background:rgba(255,250,252,0.76)!important;backdrop-filter:blur(16px) saturate(115%);border:1px solid rgba(255,255,255,0.25)!important;border-radius:14px!important;box-shadow:0 10px 26px rgba(10,6,9,0.10)!important}}
body main.main-surface [data-message-author-role="assistant"],body main.main-surface [data-message-role="assistant"]{{background:rgba(255,250,252,0.18)!important;border-radius:12px!important}}
body main.main-surface [class*="border-token-border"],body aside.app-shell-left-panel [class*="border-token-border"],body [data-codex-output-panel] [class*="border-token-border"]{{border-color:rgba(255,255,255,0.14)!important}}
body main.main-surface [data-content-search-unit-key$=":assistant"]>[data-response-annotation-target]{{color:var(--codex-assistant-theme-reading-text)!important;background:rgba(255,250,252,0.76)!important;backdrop-filter:blur(16px) saturate(115%);border:1px solid rgba(255,255,255,0.25)!important;border-radius:14px!important;box-shadow:0 10px 26px rgba(10,6,9,0.10)!important;padding:14px 16px}}
body main.main-surface [data-local-conversation-item-target-ids]{{color:var(--codex-assistant-theme-reading-text)!important;background:rgba(255,250,252,0.68)!important;backdrop-filter:blur(14px) saturate(112%);border:1px solid rgba(255,255,255,0.24)!important;border-radius:12px!important;box-shadow:0 8px 22px rgba(10,6,9,0.09)!important}}
body main.main-surface [data-local-conversation-item-target-ids] :where(p,span,a,button,[role="button"],code,strong,em){{color:var(--codex-assistant-theme-reading-text)!important}}
body [data-codex-output-panel] header[class*="bg-token-dropdown-background"],body [data-testid*="output-panel"] header[class*="bg-token-dropdown-background"],body [class*="origin-top-right"] [class*="bg-token-dropdown-background"] header[class*="bg-token-dropdown-background"]{{color:var(--codex-assistant-theme-chrome-text)!important;background:rgba(255,255,255,0.07)!important}}
body main.main-surface .composer-surface-chrome[class*="bg-token-input-background"],body main.main-surface .composer-surface-chrome[class*="bg-token-dropdown-background"]{{color:var(--codex-assistant-theme-chrome-text)!important;background:rgba(29,22,28,0.72)!important;backdrop-filter:blur(22px) saturate(135%)!important;border-width:1px!important;border-style:solid!important;border-color:rgba(255,255,255,0.14)!important;border-radius:16px!important;box-shadow:0 18px 42px rgba(10,6,9,0.22),inset 0 1px 0 rgba(255,255,255,0.08)!important}}
body main.main-surface .composer-surface-chrome :where(button,[role="button"],svg){{color:var(--codex-assistant-theme-chrome-text)!important}}
[data-codex-assistant-theme-welcome]{{position:absolute;inset:68px 0 132px;z-index:2;display:grid;align-content:center;gap:30px;padding:clamp(28px,5vw,76px);pointer-events:none}}
[data-codex-assistant-welcome-copy]{{width:min(920px,100%);margin:0 auto;color:var(--codex-assistant-theme-chrome-text);text-shadow:0 2px 18px rgba(10,6,9,0.34)}}
[data-codex-assistant-welcome-copy] h2{{margin:0;font-size:clamp(34px,4vw,58px);font-weight:720;letter-spacing:-0.035em;line-height:1.05}}
[data-codex-assistant-welcome-copy] p{{margin:12px 0 0;color:rgba(255,248,251,0.78);font-size:clamp(14px,1.3vw,18px)}}
[data-codex-assistant-welcome-grid]{{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:14px;width:min(920px,100%);margin:0 auto}}
[data-codex-assistant-welcome-action]{{pointer-events:auto;min-height:126px;padding:20px;border:1px solid rgba(255,255,255,0.14);border-radius:14px;color:var(--codex-assistant-theme-chrome-text);background:rgba(31,21,28,0.52);backdrop-filter:blur(18px) saturate(135%);box-shadow:0 12px 30px rgba(10,6,9,0.18);text-align:left;cursor:pointer;transition:transform 180ms ease,border-color 180ms ease,background-color 180ms ease,box-shadow 180ms ease}}
[data-codex-assistant-welcome-action] strong,[data-codex-assistant-welcome-action] span{{display:block;color:inherit}}
[data-codex-assistant-welcome-action] strong{{font-size:16px;font-weight:650}}
[data-codex-assistant-welcome-action] span{{margin-top:10px;color:rgba(255,238,244,0.68);font-size:13px;line-height:1.45}}
[data-codex-assistant-welcome-action]:hover{{transform:translateY(-4px);border-color:rgba(232,167,184,0.46);background:rgba(43,28,39,0.66);box-shadow:0 18px 38px rgba(10,6,9,0.24)}}
button:focus-visible,a:focus-visible,input:focus-visible,textarea:focus-visible,[role="button"]:focus-visible{{outline:2px solid rgba(232,167,184,0.80)!important;outline-offset:2px}}
@media(max-width:1200px){{:root{{--codex-assistant-theme-focal-x:{narrow_focal_x}%}}body::after{{background:linear-gradient(90deg,rgba(18,12,17,0.24) 0%,rgba(18,12,17,0.10) 50%,rgba(18,12,17,0.04) 76%,rgba(18,12,17,0.14) 100%)}}[data-codex-assistant-welcome-grid]{{grid-template-columns:repeat(2,minmax(0,1fr));max-width:660px}}[data-codex-assistant-theme-welcome]{{gap:22px;padding:28px 38px 34px}}}}
@media(max-width:760px){{[data-codex-assistant-welcome-grid]{{grid-template-columns:1fr}}[data-codex-assistant-welcome-action]{{min-height:88px}}[data-codex-assistant-theme-welcome]{{inset:56px 0 122px;overflow:auto}}}}
@media(min-aspect-ratio:21/9){{:root{{--codex-assistant-theme-focal-x:{ultrawide_focal_x}%;--codex-assistant-theme-focal-y:{ultrawide_focal_y}%}}}}
@media(prefers-reduced-motion:reduce){{*,*::before,*::after{{animation-duration:0.01ms!important;animation-iteration-count:1!important;transition-duration:0.01ms!important}}[data-codex-assistant-welcome-action]:hover{{transform:none}}}}"#,
        surface = pack.palette.surface,
        surface_strong = pack.palette.surface_strong,
        border = pack.palette.border,
        accent = pack.palette.accent,
        contrast = pack.effects.contrast_percent,
        focal_x = focal_x,
        focal_y = focal_y,
        background_size = background_size,
        background_repeat = background_repeat,
        background_position = background_position,
        narrow_focal_x = narrow_focal_x,
        ultrawide_focal_x = ultrawide_focal_x,
        ultrawide_focal_y = ultrawide_focal_y,
    );
    let theme_id =
        serde_json::to_string(&pack.id).map_err(|_| ThemeValidationError::InvalidMetadata)?;
    let css = serde_json::to_string(&css).map_err(|_| ThemeValidationError::InvalidAppearance)?;
    let source = format!(
        r#"(()=>{{"use strict";const NAME="__codexAssistantThemeV1";const PAGE_ATTRIBUTE="data-codex-assistant-page-class";const themeable=pageClass=>pageClass==="compatible-main"||pageClass==="compatible-shell";const old=globalThis[NAME];if(old&&typeof old.destroy==="function")old.destroy();const classify={classifier};const pageClass=classify();if(!themeable(pageClass))return false;const style=document.createElement("style");style.setAttribute("data-codex-assistant-theme",{theme_id});style.replaceChildren(document.createTextNode({css}));document.documentElement.append(style);const enhance={enhancer};const chrome=enhance();const sync=()=>{{const currentClass=classify();const active=themeable(currentClass);style.disabled=!active;chrome.sync(active);if(active)document.documentElement.setAttribute(PAGE_ATTRIBUTE,currentClass);else document.documentElement.removeAttribute(PAGE_ATTRIBUTE)}};sync();const observer=new MutationObserver(sync);observer.observe(document.body,{{childList:true,subtree:true,attributes:true,attributeFilter:["hidden","aria-hidden","data-state","data-page-kind","data-codex-home-state"]}});const api=Object.freeze({{id:{theme_id},pageClass,destroy(){{observer.disconnect();chrome.destroy();style.remove();document.documentElement.removeAttribute(PAGE_ATTRIBUTE);if(globalThis[NAME]===api)delete globalThis[NAME]}}}});globalThis[NAME]=api;matchMedia("(prefers-reduced-motion: reduce)");return true}})()"#,
        classifier = PAGE_CLASSIFIER,
        enhancer = THEME_ENHANCER,
    );
    if source.len() > MAX_THEME_SOURCE_BYTES {
        return Err(ThemeValidationError::InvalidAsset);
    }
    Ok(source)
}

pub async fn apply_theme_on_pages_for_version(
    endpoint: &BrowserEndpoint,
    codex_version: &str,
    pack: &ThemePack,
    previous_scripts: &[ThemeScriptRegistration],
    timeout_ms: u64,
) -> Result<ThemeApplyResult, ThemeEngineError> {
    apply_theme_on_pages_with_asset_for_version(
        endpoint,
        codex_version,
        pack,
        None,
        previous_scripts,
        timeout_ms,
    )
    .await
}

pub async fn apply_theme_on_pages_with_asset_for_version(
    endpoint: &BrowserEndpoint,
    codex_version: &str,
    pack: &ThemePack,
    image_bytes: Option<&[u8]>,
    previous_scripts: &[ThemeScriptRegistration],
    timeout_ms: u64,
) -> Result<ThemeApplyResult, ThemeEngineError> {
    select_theme_adapter(codex_version).ok_or(ThemeEngineError::UnsupportedVersion)?;
    let source = if image_bytes.is_some() {
        theme_application_source_with_asset(pack, image_bytes)
    } else {
        theme_application_source(pack)
    }
    .map_err(ThemeEngineError::InvalidPack)?;
    let verification = theme_verification_source(pack).map_err(ThemeEngineError::InvalidPack)?;
    let classification = theme_page_classification_source();
    let targets = fetch_page_targets(endpoint, timeout_ms)
        .await
        .map_err(ThemeEngineError::Discovery)?;
    let mut compatible_targets = Vec::new();
    for target in &targets {
        let mut client = CdpClient::connect_target(target, endpoint.port(), timeout_ms)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        if client
            .evaluate_boolean(&format!(
                "(()=>{{const pageClass={classification};return pageClass===\"compatible-main\"||pageClass===\"compatible-shell\"}})()"
            ))
            .await
            .map_err(ThemeEngineError::Cdp)?
        {
            compatible_targets.push(target.clone());
        }
    }
    if compatible_targets.is_empty() {
        return Err(ThemeEngineError::DomIncompatible);
    }
    if compatible_targets.len() != 1 {
        return Err(ThemeEngineError::AmbiguousPrimaryTarget);
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
            tolerate_stale_script_removal(
                client
                    .call(
                        "Page.removeScriptToEvaluateOnNewDocument",
                        serde_json::json!({"identifier": previous.identifier}),
                    )
                    .await,
            )?;
        }
        let identifier = client
            .register_script(&source)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        let atomic_application = format!(
            "(()=>{{const inserted=({source});return Boolean(inserted&&({verification}))}})()"
        );
        let visible = client
            .evaluate_boolean(&atomic_application)
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
            tolerate_stale_script_removal(
                client
                    .call(
                        "Page.removeScriptToEvaluateOnNewDocument",
                        serde_json::json!({"identifier": script.identifier}),
                    )
                    .await,
            )?;
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
    r#"(()=>{"use strict";const NAME="__codexAssistantThemeV1";const current=globalThis[NAME];if(current&&typeof current.destroy==="function")current.destroy();document.querySelector("style[data-codex-assistant-theme]")?.remove();document.querySelector("[data-codex-assistant-theme-welcome]")?.remove();document.documentElement.removeAttribute("data-codex-assistant-page-class");document.documentElement.removeAttribute("data-codex-assistant-theme-home");return true})()"#
}

fn tolerate_stale_script_removal(
    result: Result<(), CdpClientError>,
) -> Result<(), ThemeEngineError> {
    match result {
        Ok(()) | Err(CdpClientError::RemoteFailure) => Ok(()),
        Err(error) => Err(ThemeEngineError::Cdp(error)),
    }
}

pub fn theme_verification_source(pack: &ThemePack) -> Result<String, ThemeValidationError> {
    validate_theme_pack(pack, false)?;
    let theme_id =
        serde_json::to_string(&pack.id).map_err(|_| ThemeValidationError::InvalidMetadata)?;
    let backdrop_check = match &pack.backdrop {
        ThemeBackdrop::Gradient { .. } => {
            r#"backgroundImage.includes("linear-gradient")"#.to_owned()
        }
        ThemeBackdrop::Image { .. } => r#"backgroundImage.includes("data:image/")&&!backgroundImage.includes("app://")&&!backgroundImage.includes("file://")&&backgroundImage.split("data:image/").length-1===1&&backdropStyle.backgroundSize==="cover"&&backdropStyle.backgroundRepeat==="no-repeat"&&backdropStyle.pointerEvents==="none"&&backdropStyle.filter.includes("brightness")"#.to_owned(),
    };
    Ok(format!(
        r#"(()=>{{"use strict";const id={theme_id};const themeable=pageClass=>pageClass==="compatible-main"||pageClass==="compatible-shell";const api=globalThis.__codexAssistantThemeV1;const style=document.querySelector(`style[data-codex-assistant-theme="${{id}}"]`);const main=document.querySelector("main.main-surface,main[role=main],[data-codex-main=true],section[role=main]");const sidebar=document.querySelector("aside.app-shell-left-panel,aside[aria-label],nav[aria-label],[data-codex-sidebar=true]");const currentClass=document.documentElement.getAttribute("data-codex-assistant-page-class");if(!api||api.id!==id||!themeable(api.pageClass)||!style||style.disabled||!style.isConnected||!style.sheet||style.sheet.cssRules.length===0||!themeable(currentClass)||!main||!sidebar)return false;const backdropStyle=getComputedStyle(document.body,"::before");const backgroundImage=backdropStyle.backgroundImage;const mainStyle=getComputedStyle(main);const mainSurfaceTransparent=mainStyle.backgroundColor==="rgba(0, 0, 0, 0)"&&mainStyle.backgroundImage==="none";const mainRect=main.getBoundingClientRect();const sidebarRect=sidebar.getBoundingClientRect();const composer=main.querySelector(".composer-surface-chrome,form[aria-label*=message i],form[data-codex-composer=true]");let composerOkay=true;if(composer){{const rect=composer.getBoundingClientRect();const x=Math.min(innerWidth-1,Math.max(0,rect.left+rect.width/2));const y=Math.min(innerHeight-1,Math.max(0,rect.top+rect.height/2));const hit=document.elementFromPoint(x,y);composerOkay=rect.width>0&&rect.height>0&&Boolean(hit&&(hit===composer||composer.contains(hit)))}}return Boolean({backdrop_check}&&mainSurfaceTransparent&&mainRect.width>0&&mainRect.height>0&&sidebarRect.width>0&&sidebarRect.height>0&&composerOkay)}})()"#
    ))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn asset_bytes(asset_id: &str) -> Option<&'static [u8]> {
    match asset_id {
        "wisteria-bride" => Some(WISTERIA_BRIDE),
        "mint-gentleman" => Some(MINT_GENTLEMAN),
        "iris-gentleman" => Some(IRIS_GENTLEMAN),
        "crimson-palace" => Some(CRIMSON_PALACE),
        "verdant-fairy" => Some(VERDANT_FAIRY),
        "desert-prince" => Some(DESERT_PRINCE),
        "oasis-prince" => Some(OASIS_PRINCE),
        "sakura-moon" => Some(SAKURA_MOON),
        "seaside-blue" => Some(SEASIDE_BLUE),
        "autumn-wuxia" => Some(AUTUMN_WUXIA),
        "meteor-evening" => Some(METEOR_EVENING),
        "violet-blade" => Some(VIOLET_BLADE),
        "fuji-autumn" => Some(FUJI_AUTUMN),
        "spring-street" => Some(SPRING_STREET),
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
