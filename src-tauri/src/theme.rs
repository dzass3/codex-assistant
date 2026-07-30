use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::control_layer::cdp::{
    fetch_page_targets, BrowserEndpoint, CdpClient, CdpClientError, CdpDiscoveryError,
    VerifiedTarget,
};

const ENGINE_VERSION: u32 = 1;
const MAX_THEME_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_RUNTIME_THEME_ASSET_BYTES: u64 = 1_450_000;
const THEME_VERIFICATION_FRAMES: usize = 4;
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
const FUJI_AUTUMN: &[u8] = include_bytes!("../resources/themes/fuji-autumn.webp");
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemeAdaptation {
    pub luminance: u8,
    pub complexity: u8,
    pub saturation: u8,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeGenre {
    Anime,
    Fantasy,
    Nature,
    Cyber,
    Minimal,
    Dark,
    Space,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeEditorialBadge {
    Popular,
    Featured,
    New,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemeMarketplaceMetadata {
    pub genres: Vec<ThemeGenre>,
    pub badges: Vec<ThemeEditorialBadge>,
    pub published_at: String,
    pub sort_order: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ThemePack {
    pub schema_version: u32,
    pub minimum_engine_version: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ThemeCategory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace: Option<ThemeMarketplaceMetadata>,
    pub preview_path: String,
    pub backdrop: ThemeBackdrop,
    pub palette: ThemePalette,
    pub effects: ThemeEffects,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptation: Option<ThemeAdaptation>,
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

/// A committed renderer registration with read-only public metadata.
///
/// ```compile_fail
/// use codex_assistant_lib::theme::ThemeScriptRegistration;
///
/// fn cannot_change_committed_identifier(registration: &mut ThemeScriptRegistration) {
///     registration.identifier = "forged-script".to_owned();
/// }
/// ```
#[derive(Clone)]
pub struct ThemeScriptRegistration {
    target_id: String,
    identifier: String,
    source: Arc<str>,
    pending_cleanup: Vec<String>,
}

impl std::fmt::Debug for ThemeScriptRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ThemeScriptRegistration")
            .field("target_id", &self.target_id)
            .field("identifier", &self.identifier)
            .finish()
    }
}

impl PartialEq for ThemeScriptRegistration {
    fn eq(&self, other: &Self) -> bool {
        self.target_id == other.target_id && self.identifier == other.identifier
    }
}

impl Eq for ThemeScriptRegistration {}

impl ThemeScriptRegistration {
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    fn committed(
        target_id: String,
        identifier: String,
        source: Arc<str>,
        pending_cleanup: Vec<String>,
    ) -> Self {
        Self {
            target_id,
            identifier,
            source,
            pending_cleanup,
        }
    }

    fn identifiers(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.identifier.as_str())
            .chain(self.pending_cleanup.iter().map(String::as_str))
    }
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
        || pack.adaptation.is_some_and(|profile| {
            profile.luminance > 100 || profile.complexity > 100 || profile.saturation > 100
        })
        || (bundled && pack.adaptation.is_none())
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
    let profile = pack.adaptation.unwrap_or(ThemeAdaptation {
        luminance: 55,
        complexity: 60,
        saturation: 50,
    });
    let alpha = |percent: u8| format!("{}.{:02}", percent / 100, percent % 100);
    let overlay_strong =
        (30 + profile.luminance / 4 + profile.complexity / 8 + profile.saturation / 12)
            .clamp(34, 62);
    let overlay_medium = overlay_strong.saturating_sub(14).clamp(20, 48);
    let overlay_light = overlay_strong.saturating_sub(28).clamp(8, 32);
    let surface_one = (72 + profile.luminance / 8 + profile.complexity / 12).clamp(74, 86);
    let surface_two = (84 + profile.luminance / 20 + profile.complexity / 16).clamp(85, 91);
    let surface_three = (90 + profile.luminance / 25 + profile.complexity / 20).clamp(91, 96);
    let reading_surface = (89 + profile.luminance / 18 + profile.complexity / 18).clamp(91, 96);
    let sidebar_blur = (18 + profile.complexity / 8).clamp(18, 24);
    let floating_blur = (16 + profile.complexity / 10).clamp(16, 22);
    let background_brightness = (100_i16 - i16::from(profile.luminance) / 5).clamp(84, 98);
    let background_saturation = (100_i16 - i16::from(profile.saturation) / 4).clamp(80, 100);
    let background_contrast = (100 + profile.complexity / 8).clamp(101, 108);
    let css = format!(
        r#":root{{--codex-assistant-background-luminance:{luminance};--codex-assistant-background-complexity:{complexity};--codex-assistant-background-saturation:{saturation};--bg-overlay-strong:rgba(7,11,19,{overlay_strong});--bg-overlay-medium:rgba(10,14,23,{overlay_medium});--bg-overlay-light:rgba(18,20,31,{overlay_light});--surface-1:rgba(12,16,24,{surface_one});--surface-2:rgba(20,26,36,{surface_two});--surface-3:rgba(29,37,49,{surface_three});--surface-hover:rgba(255,255,255,0.09);--surface-active:color-mix(in srgb,{accent} 24%,transparent);--text-primary:rgba(248,249,252,0.98);--text-secondary:rgba(232,236,243,0.80);--text-muted:rgba(214,220,230,0.62);--accent-primary:{accent};--accent-success:#55D89F;--accent-warning:#F2AA62;--accent-danger:#F07882;--accent-info:#6DBAF8;--opacity-sidebar:{surface_one};--opacity-header:{surface_two};--opacity-card:{reading_surface};--opacity-popover:0.96;--blur-sidebar:{sidebar_blur}px;--blur-header:0px;--blur-card:0px;--blur-popover:0px;--blur-floating:{floating_blur}px;--shadow-surface:0 14px 36px rgba(4,7,13,0.28);--shadow-card:0 10px 26px rgba(4,7,13,0.16);--shadow-floating:0 20px 52px rgba(3,5,10,0.38);--radius-panel:16px;--radius-card:14px;--radius-chip:999px;--radius-input:18px;--codex-assistant-theme-surface:{surface};--codex-assistant-theme-surface-strong:{surface_strong};--codex-assistant-theme-accent:var(--accent-primary);--codex-assistant-theme-border:{border};--codex-assistant-theme-contrast:{contrast}%;--codex-assistant-theme-chrome:var(--surface-1);--codex-assistant-theme-chrome-strong:var(--surface-2);--codex-assistant-theme-chrome-text:rgba(248,249,252,0.98);--codex-assistant-theme-chrome-muted:rgba(214,220,230,0.72);--codex-assistant-theme-reading:rgba(248,249,252,{reading_surface});--codex-assistant-theme-reading-text:#242A32;--codex-assistant-theme-line:rgba(255,255,255,0.13);--codex-assistant-theme-shadow:rgba(4,7,13,0.24);--codex-assistant-theme-focal-x:{focal_x}%;--codex-assistant-theme-focal-y:{focal_y}%}}
html:root{{background-color:#0A0E16;scrollbar-color:rgba(202,211,225,0.28) transparent;scrollbar-width:thin}}
body{{min-height:100vh;position:relative;isolation:isolate}}
body,#root{{background:transparent!important}}
body::before{{content:"";position:fixed;inset:0;background-color:#0A0E16;background-image:{background};background-size:{background_size};background-repeat:{background_repeat};background-position:{background_position};background-attachment:fixed;filter:brightness({background_brightness}%) saturate({background_saturation}%) contrast({background_contrast}%);opacity:1;pointer-events:none;z-index:-2}}
body::after{{content:"";position:fixed;inset:0;background:linear-gradient(to top,rgba(5,8,14,0.46) 0%,rgba(5,8,14,0.12) 28%,transparent 52%),linear-gradient(90deg,var(--bg-overlay-strong) 0%,var(--bg-overlay-medium) 24%,var(--bg-overlay-light) 60%,rgba(7,10,17,0.12) 78%,var(--bg-overlay-medium) 100%);pointer-events:none;z-index:-1}}
::selection{{background:color-mix(in srgb,var(--accent-primary) 38%,transparent)}}
body #root>div,body main.main-surface,body main[role="main"],body [data-codex-main="true"],body section[role="main"],body .app-shell-main-content-viewport,body [class*="main-layout"],body [class*="main-content"],body [class*="chat-page"],body [class*="chat-scroll-container"],body [class*="conversation-container"]{{background:transparent!important}}
body main.main-surface,body main[role="main"],body [data-codex-main="true"],body section[role="main"]{{position:relative}}
html[data-codex-assistant-page-class="compatible-shell"] body:has([data-settings-panel-slug]) main.main-surface .app-shell-main-content-viewport .main-surface.flex.h-full.min-h-0.flex-col{{--color-token-foreground:var(--text-primary)!important;--color-token-description-foreground:var(--text-secondary)!important;--color-token-text-primary:var(--text-primary)!important;--color-token-text-secondary:var(--text-secondary)!important;--color-token-text-tertiary:var(--text-muted)!important;--color-token-icon-foreground:var(--text-primary)!important;--color-token-border:rgba(255,255,255,0.10)!important;color:var(--text-primary)!important;background:var(--surface-1)!important;backdrop-filter:none!important;border-top:1px solid rgba(255,255,255,0.08)!important;box-shadow:var(--shadow-surface)!important}}
html[data-codex-assistant-page-class="compatible-shell"] body:has([data-settings-panel-slug]) main.main-surface .app-shell-main-content-viewport .main-surface.flex.h-full.min-h-0.flex-col :where([class*="rounded-2xl"][class*="border-token-border"],[role="group"],fieldset){{color:var(--text-primary)!important;background:var(--surface-2)!important;border-color:rgba(255,255,255,0.10)!important;box-shadow:var(--shadow-card)!important}}
html[data-codex-assistant-page-class="compatible-shell"] body:has([data-settings-panel-slug]) main.main-surface .app-shell-main-content-viewport .main-surface.flex.h-full.min-h-0.flex-col [class*="bg-token-bg-fog"]{{color:var(--text-primary)!important;background:var(--surface-3)!important;border-color:rgba(255,255,255,0.12)!important;box-shadow:none!important;transition:background-color 160ms ease,border-color 160ms ease,box-shadow 160ms ease}}
html[data-codex-assistant-page-class="compatible-shell"] body:has([data-settings-panel-slug]) main.main-surface .app-shell-main-content-viewport .main-surface.flex.h-full.min-h-0.flex-col [class*="bg-token-bg-fog"]:hover{{background:var(--surface-hover)!important;border-color:rgba(255,255,255,0.18)!important}}
html[data-codex-assistant-page-class="compatible-shell"] body:has([data-settings-panel-slug]) main.main-surface .app-shell-main-content-viewport .main-surface.flex.h-full.min-h-0.flex-col [class*="bg-token-bg-fog"][data-state="open"]{{background:var(--surface-active)!important;border-color:color-mix(in srgb,var(--accent-primary) 58%,white 10%)!important;box-shadow:0 8px 24px rgba(3,5,10,0.30)!important}}
body aside.app-shell-left-panel,body [data-codex-sidebar="true"],body .app-header-tint:not([class*="group/application-menu-top-bar"]),body header[role="banner"],body [data-codex-header="true"],body [data-codex-output-panel],body [data-testid*="output-panel"],body aside[aria-label*="output" i],body aside[aria-label*="输出" i],body [class*="right-panel"],body [class*="artifact-panel"],body [class*="origin-top-right"][class*="pointer-events-none"]>[class*="pointer-events-auto"]>[class*="bg-token-dropdown-background"],body main .composer-surface-chrome,body main form[aria-label*="message" i],body main form[data-codex-composer="true"],body :where([role="menu"],[role="listbox"],[data-radix-popper-content-wrapper] [class*="bg-token-dropdown-background"]){{--color-token-foreground:var(--text-primary)!important;--color-token-description-foreground:var(--text-secondary)!important;--color-token-conversation-body:var(--text-primary)!important;--color-token-conversation-summary-leading:var(--text-secondary)!important;--color-token-conversation-summary-trailing:var(--text-muted)!important;--color-token-text-primary:var(--text-primary)!important;--color-token-text-secondary:var(--text-secondary)!important;--color-token-text-tertiary:var(--text-muted)!important;--color-token-editor-foreground:var(--text-primary)!important;--color-token-icon-foreground:var(--text-primary)!important;--color-token-application-menu-foreground:var(--text-primary)!important;--color-token-dropdown-foreground:var(--text-primary)!important;--color-token-input-foreground:var(--text-primary)!important;--color-text-button-secondary:var(--text-primary)!important;--vscode-foreground:var(--text-primary)!important;--vscode-sideBar-foreground:var(--text-primary)!important;--vscode-menu-foreground:var(--text-primary)!important;--vscode-input-foreground:var(--text-primary)!important}}
body aside.app-shell-left-panel,body aside[aria-label]:not([data-codex-output-panel]):not([aria-label*="output" i]):not([aria-label*="输出" i]),body [data-codex-sidebar="true"]{{color:var(--text-primary)!important;background:var(--surface-1)!important;backdrop-filter:none!important;border-right:1px solid rgba(255,255,255,0.09)!important;box-shadow:var(--shadow-surface)!important;scrollbar-width:thin;scrollbar-color:rgba(202,211,225,0.25) transparent}}
body aside.app-shell-left-panel :where(h1,h2,h3,h4,a,button,[role="button"],label),body nav[aria-label] :where(h1,h2,h3,h4,a,button,[role="button"],label),body [data-codex-sidebar="true"] :where(h1,h2,h3,h4,a,button,[role="button"],label){{color:var(--text-primary)!important}}
body aside.app-shell-left-panel :where(p,span),body nav[aria-label] :where(p,span),body [data-codex-sidebar="true"] :where(p,span){{color:inherit}}
body aside.app-shell-left-panel :where(button,[role="button"],a),body nav[aria-label] :where(button,[role="button"],a),body [data-codex-sidebar="true"] :where(button,[role="button"],a),body aside.app-shell-left-panel svg,body nav[aria-label] svg,body [data-codex-sidebar="true"] svg{{color:var(--text-primary)!important}}
body aside.app-shell-left-panel svg [stroke]:not([stroke="none"]),body nav[aria-label] svg [stroke]:not([stroke="none"]),body [data-codex-sidebar="true"] svg [stroke]:not([stroke="none"]){{stroke:currentColor!important}}
body aside.app-shell-left-panel::-webkit-scrollbar,body [data-codex-sidebar="true"]::-webkit-scrollbar,body [data-codex-output-panel]::-webkit-scrollbar{{width:6px;height:6px}}
body aside.app-shell-left-panel::-webkit-scrollbar-thumb,body [data-codex-sidebar="true"]::-webkit-scrollbar-thumb,body [data-codex-output-panel]::-webkit-scrollbar-thumb{{background:rgba(202,211,225,0.24);border-radius:var(--radius-chip)}}
body aside.app-shell-left-panel :where(button,[role="button"],a),body [data-codex-sidebar="true"] :where(button,[role="button"],a){{transition:background-color 160ms ease,box-shadow 160ms ease,transform 160ms ease}}
body aside.app-shell-left-panel :where(button,[role="button"],a):hover,body [data-codex-sidebar="true"] :where(button,[role="button"],a):hover{{background:var(--surface-hover)!important;transform:translateX(1px)}}
body aside.app-shell-left-panel :where(button,[role="button"],a):active,body [data-codex-sidebar="true"] :where(button,[role="button"],a):active{{background:var(--surface-active)!important;transform:none}}
body aside.app-shell-left-panel [aria-current="page"],body [data-codex-sidebar="true"] [aria-current="page"]{{background:linear-gradient(90deg,color-mix(in srgb,var(--accent-primary) 28%,transparent),color-mix(in srgb,var(--accent-primary) 10%,transparent))!important;box-shadow:inset 3px 0 0 var(--accent-primary)!important}}
body .app-header-tint,body header[role="banner"],body [data-codex-header="true"]{{color:var(--text-primary)!important;background:var(--surface-2)!important;backdrop-filter:none!important;border-bottom:1px solid rgba(255,255,255,0.10)!important;box-shadow:0 8px 24px rgba(4,7,13,0.16)!important}}
body .app-header-tint :where(button,[role="button"],svg),body header[role="banner"] :where(button,[role="button"],svg),body [data-codex-header="true"] :where(button,[role="button"],svg){{color:var(--text-primary)!important}}
body .app-header-tint :where(button,[role="button"]),body header[role="banner"] :where(button,[role="button"]),body [data-codex-header="true"] :where(button,[role="button"]){{background:var(--surface-3)!important;border:1px solid rgba(255,255,255,0.08)!important;border-radius:var(--radius-chip)!important;transition:background-color 160ms ease,border-color 160ms ease,transform 160ms ease}}
body .app-header-tint :where(button,[role="button"]):hover,body header[role="banner"] :where(button,[role="button"]):hover,body [data-codex-header="true"] :where(button,[role="button"]):hover{{background:var(--surface-hover)!important;border-color:rgba(255,255,255,0.16)!important}}
body .app-header-tint :where(button,[role="button"]):active,body header[role="banner"] :where(button,[role="button"]):active,body [data-codex-header="true"] :where(button,[role="button"]):active{{transform:translateY(1px);background:var(--surface-active)!important}}
body .app-header-tint[class*="group/application-menu-top-bar"]{{--color-token-foreground:#242A32!important;--color-token-description-foreground:rgba(36,42,50,0.68)!important;--color-token-text-secondary:rgba(36,42,50,0.72)!important;--color-token-text-tertiary:rgba(36,42,50,0.62)!important;--color-token-icon-foreground:#242A32!important;--color-token-application-menu-foreground:#242A32!important;--color-text-button-secondary:#242A32!important;color:#242A32!important;background:rgba(248,249,252,0.94)!important;backdrop-filter:none!important;border-bottom-color:rgba(36,42,50,0.12)!important;box-shadow:0 4px 14px rgba(4,7,13,0.10)!important}}
body .app-header-tint[class*="group/application-menu-top-bar"] :where(strong,span,p,button,[role="button"],svg){{color:#242A32!important}}
body .app-header-tint[class*="group/application-menu-top-bar"] :where(button,[role="button"]){{background:transparent!important;border-color:transparent!important;box-shadow:none!important}}
body .app-header-tint[class*="group/application-menu-top-bar"] :where(button,[role="button"]):hover{{background:rgba(36,42,50,0.08)!important;border-color:transparent!important}}
body .app-header-tint[class*="group/application-menu-top-bar"] :where(button,[role="button"]):active{{background:rgba(36,42,50,0.12)!important;transform:translateY(1px)}}
body [data-codex-output-panel],body [data-testid*="output-panel"],body aside[aria-label*="output" i],body aside[aria-label*="输出" i],body [class*="right-panel"],body [class*="artifact-panel"],body [class*="origin-top-right"][class*="pointer-events-none"]>[class*="pointer-events-auto"]>[class*="bg-token-dropdown-background"]{{color:var(--text-primary)!important;background:var(--surface-1)!important;backdrop-filter:none!important;border:1px solid rgba(255,255,255,0.10)!important;border-radius:var(--radius-panel)!important;box-shadow:var(--shadow-surface)!important;scrollbar-width:thin;scrollbar-color:rgba(202,211,225,0.25) transparent}}
body [data-codex-output-panel] :where(strong,h1,h2,h3,h4,a,button,[role="button"],svg),body [data-testid*="output-panel"] :where(strong,h1,h2,h3,h4,a,button,[role="button"],svg),body [class*="right-panel"] :where(strong,h1,h2,h3,h4,a,button,[role="button"],svg),body [class*="artifact-panel"] :where(strong,h1,h2,h3,h4,a,button,[role="button"],svg),body [class*="origin-top-right"][class*="pointer-events-none"]>[class*="pointer-events-auto"]>[class*="bg-token-dropdown-background"] :where(strong,h1,h2,h3,h4,a,button,[role="button"],svg){{color:var(--text-primary)!important}}
body [data-codex-output-panel] :where(p,span,li,time,code,small),body [data-testid*="output-panel"] :where(p,span,li,time,code,small),body [class*="right-panel"] :where(p,span,li,time,code,small),body [class*="artifact-panel"] :where(p,span,li,time,code,small),body [class*="origin-top-right"][class*="pointer-events-none"]>[class*="pointer-events-auto"]>[class*="bg-token-dropdown-background"] :where(p,span,li,time,code,small){{color:var(--text-secondary)!important}}
body [data-codex-output-panel] :where(button,[role="button"],a) :where(span,strong,svg),body [data-testid*="output-panel"] :where(button,[role="button"],a) :where(span,strong,svg),body [class*="right-panel"] :where(button,[role="button"],a) :where(span,strong,svg),body [class*="artifact-panel"] :where(button,[role="button"],a) :where(span,strong,svg){{color:var(--text-primary)!important}}
body [data-codex-output-panel] :where(section,[role="group"],li),body [data-testid*="output-panel"] :where(section,[role="group"],li),body [class*="right-panel"] :where(section,[role="group"],li),body [class*="artifact-panel"] :where(section,[role="group"],li){{background:var(--surface-2)!important;border:1px solid rgba(255,255,255,0.08)!important;border-radius:var(--radius-card)!important;box-shadow:var(--shadow-card)!important}}
body [data-codex-output-panel] :where(button,[role="button"],a),body [data-testid*="output-panel"] :where(button,[role="button"],a),body [class*="right-panel"] :where(button,[role="button"],a),body [class*="artifact-panel"] :where(button,[role="button"],a){{transition:background-color 180ms ease,border-color 180ms ease}}
body [data-codex-output-panel] :where(button,[role="button"],a):hover,body [data-testid*="output-panel"] :where(button,[role="button"],a):hover,body [class*="right-panel"] :where(button,[role="button"],a):hover,body [class*="artifact-panel"] :where(button,[role="button"],a):hover{{background:var(--surface-hover)!important}}
body main .composer-surface-chrome,body main form[aria-label*="message" i],body main form[data-codex-composer="true"],body main.main-surface .composer-surface-chrome[class*="bg-token-input-background"],body main.main-surface .composer-surface-chrome[class*="bg-token-dropdown-background"]{{color:var(--text-primary)!important;background:var(--surface-3)!important;backdrop-filter:none!important;border:1px solid rgba(255,255,255,0.15)!important;border-radius:var(--radius-input)!important;box-shadow:var(--shadow-floating),inset 0 1px 0 rgba(255,255,255,0.08)!important}}
body main .composer-surface-chrome :where(textarea,input,[contenteditable="true"]),body main form[aria-label*="message" i] :where(textarea,input,[contenteditable="true"]),body main form[data-codex-composer="true"] :where(textarea,input,[contenteditable="true"]){{color:var(--text-primary)!important;caret-color:var(--accent-primary)!important;background:transparent!important}}
body main .composer-surface-chrome :where(textarea,input)::placeholder,body main form[aria-label*="message" i] :where(textarea,input)::placeholder,body main form[data-codex-composer="true"] :where(textarea,input)::placeholder{{color:var(--text-muted)!important;opacity:1}}
body main .composer-surface-chrome :where(button,[role="button"],[aria-haspopup="listbox"],[aria-haspopup="menu"]),body main form[aria-label*="message" i] :where(button,[role="button"],[aria-haspopup="listbox"],[aria-haspopup="menu"]){{color:var(--text-primary)!important;background:rgba(255,255,255,0.07)!important;border:1px solid rgba(255,255,255,0.08)!important;border-radius:var(--radius-chip)!important;transition:background-color 160ms ease,border-color 160ms ease,transform 160ms ease}}
body main .composer-surface-chrome [class*="ModelPickerTriggerModelLabel"],body main .composer-surface-chrome [class*="ModelPickerTriggerModelText"],body main form[aria-label*="message" i] [class*="ModelPickerTriggerModelLabel"],body main form[aria-label*="message" i] [class*="ModelPickerTriggerModelText"]{{color:var(--text-primary)!important}}
body main .composer-surface-chrome :where(button,[role="button"],[aria-haspopup="listbox"],[aria-haspopup="menu"]):hover{{background:var(--surface-hover)!important;border-color:rgba(255,255,255,0.17)!important}}
body main .composer-surface-chrome :where(button,[role="button"],[aria-haspopup="listbox"],[aria-haspopup="menu"]):active{{transform:translateY(1px);background:var(--surface-active)!important}}
body main .composer-surface-chrome :where([aria-expanded="true"],[data-state="open"]){{background:var(--surface-active)!important;border-color:color-mix(in srgb,var(--accent-primary) 58%,white 10%)!important;box-shadow:0 8px 24px rgba(3,5,10,0.30)!important}}
body main .composer-surface-chrome:focus-within,body main form[aria-label*="message" i]:focus-within,body main form[data-codex-composer="true"]:focus-within{{border-color:color-mix(in srgb,var(--accent-primary) 72%,white 12%)!important;box-shadow:var(--shadow-floating),0 0 0 2px color-mix(in srgb,var(--accent-primary) 25%,transparent)!important}}
body main.main-surface [class*="from-token-main-surface-primary"][class*="via-token-main-surface-primary"]{{background-image:linear-gradient(to top,var(--bg-overlay-strong) 0%,var(--bg-overlay-light) 52%,transparent 100%)!important}}
body main.main-surface [class~="from-token-main-surface-primary"][class~="to-transparent"]{{background-image:linear-gradient(to top,var(--surface-1) 0%,color-mix(in srgb,var(--surface-1) 48%,transparent) 52%,transparent 100%)!important}}
body main.main-surface [class~="after:from-token-main-surface-primary"]::after{{background-image:linear-gradient(to bottom,var(--surface-1) 0%,color-mix(in srgb,var(--surface-1) 46%,transparent) 54%,transparent 100%)!important}}
body main.main-surface [class*="bg-token-main-surface-primary"]{{background-color:transparent!important}}
body main.main-surface [class*="bg-token-button-background-secondary"]{{transition:background-color 160ms ease,transform 160ms ease}}
body main.main-surface [class*="bg-token-button-background-secondary"]:hover{{background:var(--surface-hover)!important}}
body main.main-surface [class*="bg-token-button-background-secondary"]:active{{background:var(--surface-active)!important;transform:translateY(1px)}}
body :where([role="menu"],[role="listbox"],[data-radix-popper-content-wrapper] [class*="bg-token-dropdown-background"]){{color:var(--text-primary)!important;background:rgba(12,16,24,var(--opacity-popover))!important;backdrop-filter:none!important;border:1px solid rgba(255,255,255,0.13)!important;border-radius:var(--radius-card)!important;box-shadow:var(--shadow-floating)!important}}
body :where([role="menu"],[role="listbox"],[data-radix-popper-content-wrapper] [class*="bg-token-dropdown-background"]) :where(button,[role="menuitem"],[role="option"]){{color:var(--text-primary)!important}}
body :where([role="menuitem"],[role="option"]):hover,body :where([role="menuitem"],[role="option"])[data-highlighted]{{background:var(--surface-hover)!important}}
body main.main-surface [class*="bg-token-dropdown-background"]{{background:rgba(248,249,252,var(--opacity-card))!important;border:1px solid rgba(36,42,50,0.14)!important;border-radius:var(--radius-card)!important;box-shadow:var(--shadow-card)!important;backdrop-filter:none!important}}
body main .composer-surface-chrome [data-composer-attachment-pill="true"],body main form[aria-label*="message" i] [data-composer-attachment-pill="true"],body main form[data-codex-composer="true"] [data-composer-attachment-pill="true"]{{color:var(--text-primary)!important;background:var(--surface-2)!important;border:1px solid rgba(255,255,255,0.12)!important;border-radius:var(--radius-card)!important;box-shadow:var(--shadow-card)!important;backdrop-filter:none!important}}
body main .composer-surface-chrome [data-composer-attachment-pill="true"] :where(span,p,strong,small,svg,svg *),body main form[aria-label*="message" i] [data-composer-attachment-pill="true"] :where(span,p,strong,small,svg,svg *),body main form[data-codex-composer="true"] [data-composer-attachment-pill="true"] :where(span,p,strong,small,svg,svg *){{color:var(--text-primary)!important}}
body main.main-surface [data-user-message-bubble="true"],body main.main-surface [data-message-author-role="user"],body main.main-surface [data-message-role="user"],body main.main-surface [data-testid*="user-message"],body main.main-surface [data-codex-reading-surface="true"],body main.main-surface pre{{color:var(--codex-assistant-theme-reading-text)!important;background:rgba(248,249,252,var(--opacity-card))!important;backdrop-filter:none!important;border:1px solid rgba(36,42,50,0.13)!important;border-radius:var(--radius-card)!important;box-shadow:var(--shadow-card)!important}}
body main.main-surface [data-message-author-role="assistant"],body main.main-surface [data-message-role="assistant"]{{background:rgba(248,249,252,0.20)!important;border-radius:var(--radius-card)!important}}
body main.main-surface [class*="border-token-border"],body aside.app-shell-left-panel [class*="border-token-border"],body [data-codex-output-panel] [class*="border-token-border"]{{border-color:rgba(255,255,255,0.12)!important}}
body main.main-surface [data-content-search-unit-key$=":assistant"]>:first-child{{color:var(--codex-assistant-theme-reading-text)!important;background:rgba(248,249,252,var(--opacity-card))!important;backdrop-filter:none!important;border:1px solid rgba(36,42,50,0.13)!important;border-radius:var(--radius-card)!important;box-shadow:var(--shadow-card)!important;padding:16px 18px;line-height:1.62}}
body main.main-surface [data-content-search-unit-key$=":assistant"]>:first-child :where(p,ul,ol,blockquote,pre){{margin-block:0.65em}}
body main.main-surface .loading-shimmer-pure-text{{display:inline-flex!important;align-items:center;min-height:40px;width:fit-content;max-width:100%;padding:8px 12px!important;color:rgba(248,249,252,0.96)!important;-webkit-text-fill-color:currentColor!important;background:var(--surface-2)!important;backdrop-filter:none!important;border:1px solid rgba(255,255,255,0.10)!important;border-left:3px solid var(--accent-info)!important;border-radius:var(--radius-card)!important;box-shadow:var(--shadow-card)!important;animation:none!important}}
body main.main-surface .loading-shimmer-pure-text>*{{color:inherit!important;-webkit-text-fill-color:currentColor!important;animation:none!important}}
body main.main-surface [data-local-conversation-item-target-ids],body main.main-surface [data-testid*="tool"],body main.main-surface [data-testid*="status"],body main.main-surface [data-testid*="progress"]{{position:relative;min-height:44px;margin-block:8px!important;padding:10px 12px!important;line-height:1.4;color:var(--codex-assistant-theme-chrome-text)!important;background:var(--surface-2)!important;backdrop-filter:none!important;border:1px solid rgba(255,255,255,0.10)!important;border-left:3px solid var(--accent-info)!important;border-radius:var(--radius-card)!important;box-shadow:var(--shadow-card)!important;transition:background-color 160ms ease,border-color 160ms ease,box-shadow 160ms ease}}
body main.main-surface [data-local-conversation-item-target-ids],body main.main-surface [data-testid*="tool"],body main.main-surface [data-testid*="status"],body main.main-surface [data-testid*="progress"]{{--color-token-foreground:var(--codex-assistant-theme-chrome-text)!important;--color-token-description-foreground:var(--codex-assistant-theme-chrome-muted)!important;--color-token-conversation-body:var(--codex-assistant-theme-chrome-text)!important;--color-token-conversation-summary-leading:var(--codex-assistant-theme-chrome-muted)!important;--color-token-conversation-summary-trailing:var(--codex-assistant-theme-chrome-muted)!important;--color-token-text-primary:var(--codex-assistant-theme-chrome-text)!important;--color-token-text-secondary:var(--codex-assistant-theme-chrome-muted)!important;--color-token-text-tertiary:var(--codex-assistant-theme-chrome-muted)!important;--color-token-editor-foreground:var(--codex-assistant-theme-chrome-text)!important;--vscode-foreground:var(--codex-assistant-theme-chrome-text)!important}}
body main.main-surface [data-local-conversation-item-target-ids]:hover,body main.main-surface [data-testid*="tool"]:hover,body main.main-surface [data-testid*="status"]:hover{{background:var(--surface-3)!important;border-color:rgba(255,255,255,0.16)!important}}
body main.main-surface [data-state="running"],body main.main-surface [data-status="running"],body main.main-surface [aria-busy="true"]{{border-left-color:var(--accent-info)!important}}
body main.main-surface [data-state="success"],body main.main-surface [data-status="success"],body main.main-surface [data-state="completed"],body main.main-surface [data-status="completed"]{{border-left-color:var(--accent-success)!important}}
body main.main-surface [data-state="warning"],body main.main-surface [data-status="warning"]{{border-left-color:var(--accent-warning)!important}}
body main.main-surface [data-state="error"],body main.main-surface [data-status="error"],body main.main-surface [data-state="failed"],body main.main-surface [data-status="failed"]{{border-left-color:var(--accent-danger)!important}}
body main.main-surface :where([data-local-conversation-item-target-ids],[data-testid*="tool"],[data-testid*="status"]) :where(p,span,a,button,[role="button"],code,strong,em,time,summary){{color:var(--codex-assistant-theme-chrome-text)!important}}
body main.main-surface :where([data-local-conversation-item-target-ids],[data-testid*="tool"],[data-testid*="status"])>strong{{margin-right:8px;font-weight:650}}
body main.main-surface :where([data-local-conversation-item-target-ids],[data-testid*="tool"],[data-testid*="status"])>span{{color:var(--codex-assistant-theme-chrome-muted)!important}}
body main.main-surface :where(progress,[role="progressbar"]){{accent-color:var(--accent-info)}}
body main.main-surface :where(details,summary){{border-radius:var(--radius-chip)}}
body main.main-surface :where(summary):hover{{background:var(--surface-hover)!important}}
body [data-codex-output-panel] header[class*="bg-token-dropdown-background"],body [data-testid*="output-panel"] header[class*="bg-token-dropdown-background"],body [class*="origin-top-right"] [class*="bg-token-dropdown-background"] header[class*="bg-token-dropdown-background"]{{color:var(--text-primary)!important;background:var(--surface-2)!important;backdrop-filter:none!important;border-bottom:1px solid rgba(255,255,255,0.08)!important}}
body main.main-surface .composer-surface-chrome :where(button,[role="button"],svg,svg *){{color:var(--text-primary)!important}}
body :where(button,[role="button"],input,textarea,select,[aria-haspopup]):disabled,body :where(button,[role="button"],input,textarea,select,[aria-haspopup])[aria-disabled="true"]{{opacity:0.48!important;cursor:not-allowed!important;transform:none!important;box-shadow:none!important}}
[data-codex-assistant-theme-welcome]{{position:absolute;inset:68px 0 132px;z-index:2;display:grid;align-content:center;gap:30px;padding:clamp(28px,5vw,76px);pointer-events:none}}
[data-codex-assistant-welcome-copy]{{width:min(920px,100%);margin:0 auto;color:var(--text-primary);text-shadow:0 2px 18px rgba(3,5,10,0.46)}}
[data-codex-assistant-welcome-copy] h2{{margin:0;font-size:clamp(34px,4vw,58px);font-weight:720;letter-spacing:-0.035em;line-height:1.05}}
[data-codex-assistant-welcome-copy] p{{margin:12px 0 0;color:var(--text-secondary);font-size:clamp(14px,1.3vw,18px)}}
[data-codex-assistant-welcome-grid]{{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:14px;width:min(920px,100%);margin:0 auto}}
[data-codex-assistant-welcome-action]{{pointer-events:auto;min-height:126px;padding:20px;border:1px solid rgba(255,255,255,0.12);border-radius:var(--radius-card);color:var(--text-primary);background:var(--surface-2);backdrop-filter:none;box-shadow:var(--shadow-card);text-align:left;cursor:pointer;transition:transform 180ms ease,border-color 180ms ease,background-color 180ms ease,box-shadow 180ms ease}}
[data-codex-assistant-welcome-action] strong,[data-codex-assistant-welcome-action] span{{display:block;color:inherit}}
[data-codex-assistant-welcome-action] strong{{font-size:16px;font-weight:650}}
[data-codex-assistant-welcome-action] span{{margin-top:10px;color:var(--text-muted);font-size:13px;line-height:1.45}}
[data-codex-assistant-welcome-action]:hover{{transform:translateY(-4px);border-color:color-mix(in srgb,var(--accent-primary) 52%,white 8%);background:var(--surface-3);box-shadow:var(--shadow-floating)}}
button:focus-visible,a:focus-visible,input:focus-visible,textarea:focus-visible,select:focus-visible,[role="button"]:focus-visible,[aria-haspopup]:focus-visible{{outline:2px solid color-mix(in srgb,var(--accent-primary) 76%,white 12%)!important;outline-offset:2px}}
@media(max-width:1200px){{:root{{--codex-assistant-theme-focal-x:{narrow_focal_x}%}}body::after{{background:linear-gradient(to top,rgba(5,8,14,0.52) 0%,rgba(5,8,14,0.16) 30%,transparent 54%),linear-gradient(90deg,var(--bg-overlay-strong) 0%,var(--bg-overlay-medium) 54%,var(--bg-overlay-light) 78%,var(--bg-overlay-medium) 100%)}}[data-codex-assistant-welcome-grid]{{grid-template-columns:repeat(2,minmax(0,1fr));max-width:660px}}[data-codex-assistant-theme-welcome]{{gap:22px;padding:28px 38px 34px}}}}
@media(max-width:760px){{[data-codex-assistant-welcome-grid]{{grid-template-columns:1fr}}[data-codex-assistant-welcome-action]{{min-height:88px}}[data-codex-assistant-theme-welcome]{{inset:56px 0 122px;overflow:auto}}}}
@media(min-aspect-ratio:21/9){{:root{{--codex-assistant-theme-focal-x:{ultrawide_focal_x}%;--codex-assistant-theme-focal-y:{ultrawide_focal_y}%}}}}
@media(prefers-reduced-motion:reduce){{*,*::before,*::after{{animation-duration:0.01ms!important;animation-iteration-count:1!important;transition-duration:0.01ms!important}}[data-codex-assistant-welcome-action]:hover{{transform:none}}}}"#,
        surface = pack.palette.surface,
        surface_strong = pack.palette.surface_strong,
        border = pack.palette.border,
        accent = pack.palette.accent,
        contrast = pack.effects.contrast_percent,
        luminance = profile.luminance,
        complexity = profile.complexity,
        saturation = profile.saturation,
        overlay_strong = alpha(overlay_strong),
        overlay_medium = alpha(overlay_medium),
        overlay_light = alpha(overlay_light),
        surface_one = alpha(surface_one),
        surface_two = alpha(surface_two),
        surface_three = alpha(surface_three),
        reading_surface = alpha(reading_surface),
        sidebar_blur = sidebar_blur,
        floating_blur = floating_blur,
        background_brightness = background_brightness,
        background_saturation = background_saturation,
        background_contrast = background_contrast,
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
        r#"(()=>{{"use strict";const NAME="__codexAssistantThemeV1";const PAGE_ATTRIBUTE="data-codex-assistant-page-class";const HOT_CONTENT_SELECTOR='[data-content-search-unit-key],.loading-shimmer-pure-text,.composer-surface-chrome,form[aria-label*="message" i],form[data-codex-composer="true"]';const CLASSIFICATION_MARKER_SELECTOR='[data-security-prompt],[data-auth-screen],[data-account-screen],[data-payment-screen],[data-authorization-screen],[data-permission-screen],[data-recovery-screen],[data-page-kind],[data-codex-home-state],[data-codex-conversation],[data-settings-page],[data-codex-main]';const STRUCTURAL_SELECTOR='body,#root,main,main.main-surface,[role="main"],aside.app-shell-left-panel,[data-codex-sidebar]';const themeable=pageClass=>pageClass==="compatible-main"||pageClass==="compatible-shell";const old=globalThis[NAME];if(old&&typeof old.destroy==="function")old.destroy();const classify={classifier};const pageClass=classify();if(!themeable(pageClass))return false;const style=document.createElement("style");style.setAttribute("data-codex-assistant-theme",{theme_id});style.replaceChildren(document.createTextNode({css}));document.documentElement.append(style);const enhance={enhancer};const chrome=enhance();const sync=()=>{{const currentClass=classify();const active=themeable(currentClass);style.disabled=!active;chrome.sync(active);if(active)document.documentElement.setAttribute(PAGE_ATTRIBUTE,currentClass);else document.documentElement.removeAttribute(PAGE_ATTRIBUTE)}};const asElement=node=>node instanceof Element?node:null;const containsSelector=(node,selector)=>{{const element=asElement(node);return Boolean(element&&(element.matches(selector)||element.querySelector(selector)))}};const affectsClassification=records=>records.some(record=>{{const target=asElement(record.target);if(record.type==="attributes")return Boolean(target&&(target.matches(CLASSIFICATION_MARKER_SELECTOR)||!target.closest(HOT_CONTENT_SELECTOR)&&target.matches(STRUCTURAL_SELECTOR)));const changedNodes=[...record.addedNodes,...record.removedNodes];if(changedNodes.some(node=>containsSelector(node,CLASSIFICATION_MARKER_SELECTOR)))return true;if(!target||target.closest(HOT_CONTENT_SELECTOR)||changedNodes.every(node=>!asElement(node)||containsSelector(node,HOT_CONTENT_SELECTOR)))return false;return Boolean(target.matches(STRUCTURAL_SELECTOR)||target.closest(STRUCTURAL_SELECTOR))}});let scheduledFrame=0;const requestSync=()=>{{if(scheduledFrame)return;scheduledFrame=requestAnimationFrame(()=>{{scheduledFrame=0;sync()}})}};sync();const observer=new MutationObserver(records=>{{if(affectsClassification(records))requestSync()}});observer.observe(document.body,{{childList:true,subtree:true,attributes:true,attributeFilter:["hidden","aria-hidden","data-state","data-page-kind","data-codex-home-state"]}});const api=Object.freeze({{id:{theme_id},pageClass,destroy(){{observer.disconnect();if(scheduledFrame)cancelAnimationFrame(scheduledFrame);chrome.destroy();style.remove();document.documentElement.removeAttribute(PAGE_ATTRIBUTE);if(globalThis[NAME]===api)delete globalThis[NAME]}}}});globalThis[NAME]=api;matchMedia("(prefers-reduced-motion: reduce)");return true}})()"#,
        classifier = PAGE_CLASSIFIER,
        enhancer = THEME_ENHANCER,
    );
    if source.len() > MAX_THEME_SOURCE_BYTES {
        return Err(ThemeValidationError::InvalidAsset);
    }
    Ok(source)
}

struct RendererThemeTransaction<'a> {
    target: &'a VerifiedTarget,
    endpoint_port: u16,
    source: &'a str,
    verification: &'a str,
    previous: Option<&'a ThemeScriptRegistration>,
    timeout_ms: u64,
}

impl RendererThemeTransaction<'_> {
    async fn execute(self) -> Result<ThemeScriptRegistration, ThemeEngineError> {
        let mut client =
            CdpClient::connect_target(self.target, self.endpoint_port, self.timeout_ms)
                .await
                .map_err(ThemeEngineError::Cdp)?;
        client
            .call("Page.enable", serde_json::json!({}))
            .await
            .map_err(ThemeEngineError::Cdp)?;
        let identifier = client
            .register_script(self.source)
            .await
            .map_err(ThemeEngineError::Cdp)?;
        let inserted = match client.evaluate_boolean(self.source).await {
            Ok(inserted) => inserted,
            Err(error) => {
                self.rollback(&identifier).await?;
                return Err(ThemeEngineError::Cdp(error));
            }
        };
        let visible = if inserted {
            let mut verified = false;
            for _ in 0..THEME_VERIFICATION_FRAMES {
                if let Err(error) = client.evaluate_boolean_after_animation_frame("true").await {
                    self.rollback(&identifier).await?;
                    return Err(ThemeEngineError::Cdp(error));
                }
                match client.evaluate_boolean(self.verification).await {
                    Ok(true) => {
                        verified = true;
                        break;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        self.rollback(&identifier).await?;
                        return Err(ThemeEngineError::Cdp(error));
                    }
                }
            }
            verified
        } else {
            false
        };
        if !visible {
            self.rollback_with(&mut client, &identifier).await?;
            return Err(ThemeEngineError::PartialApplication);
        }

        let mut pending_cleanup = Vec::new();
        if let Some(previous) = self.previous {
            for previous_identifier in previous.identifiers() {
                if client
                    .call(
                        "Page.removeScriptToEvaluateOnNewDocument",
                        serde_json::json!({"identifier": previous_identifier}),
                    )
                    .await
                    .is_err()
                {
                    pending_cleanup.push(previous_identifier.to_owned());
                }
            }
        }
        Ok(ThemeScriptRegistration::committed(
            self.target.target_id.clone(),
            identifier,
            Arc::from(self.source),
            pending_cleanup,
        ))
    }

    async fn rollback(&self, identifier: &str) -> Result<(), ThemeEngineError> {
        let mut client =
            CdpClient::connect_target(self.target, self.endpoint_port, self.timeout_ms)
                .await
                .map_err(ThemeEngineError::Cdp)?;
        self.rollback_with(&mut client, identifier).await
    }

    async fn rollback_with(
        &self,
        client: &mut CdpClient,
        identifier: &str,
    ) -> Result<(), ThemeEngineError> {
        let removal = client
            .call(
                "Page.removeScriptToEvaluateOnNewDocument",
                serde_json::json!({"identifier": identifier}),
            )
            .await;
        let restoration = if removal.is_ok() {
            self.restore_live_renderer(client).await
        } else {
            match CdpClient::connect_target(self.target, self.endpoint_port, self.timeout_ms).await
            {
                Ok(mut restoration_client) => {
                    self.restore_live_renderer(&mut restoration_client).await
                }
                Err(error) => Err(ThemeEngineError::Cdp(error)),
            }
        };
        if let Err(error) = removal {
            return Err(ThemeEngineError::Cdp(error));
        }
        restoration
    }

    async fn restore_live_renderer(&self, client: &mut CdpClient) -> Result<(), ThemeEngineError> {
        let rollback_source = match self.previous {
            Some(previous) => previous.source.as_ref(),
            None => theme_restore_source(),
        };
        if !client
            .evaluate_boolean(rollback_source)
            .await
            .map_err(ThemeEngineError::Cdp)?
        {
            return Err(ThemeEngineError::PartialApplication);
        }
        Ok(())
    }
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
    let mut scripts = Vec::new();
    for target in compatible_targets {
        let previous = previous_scripts
            .iter()
            .find(|script| script.target_id == target.target_id);
        scripts.push(
            RendererThemeTransaction {
                target: &target,
                endpoint_port: endpoint.port(),
                source: &source,
                verification: &verification,
                previous,
                timeout_ms,
            }
            .execute()
            .await?,
        );
    }
    Ok(ThemeApplyResult {
        applied_pages: scripts.len(),
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
            for identifier in script.identifiers() {
                tolerate_stale_script_removal(
                    client
                        .call(
                            "Page.removeScriptToEvaluateOnNewDocument",
                            serde_json::json!({"identifier": identifier}),
                        )
                        .await,
                )?;
            }
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
        "fuji-autumn" => Some(FUJI_AUTUMN),
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
