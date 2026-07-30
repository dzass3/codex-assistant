use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::theme::{
    validate_theme_pack, RightsStatus, ThemeAdaptation, ThemeAsset, ThemeBackdrop, ThemeCategory,
    ThemeEffects, ThemePack, ThemePalette, ThemeRights, MAX_RUNTIME_THEME_ASSET_BYTES,
};

const LOCAL_THEME_DIRECTORY: &str = "local-themes";
const MANIFEST_FILE: &str = "theme.json";
const MAX_MANIFEST_BYTES: u64 = 32 * 1024;
const MANIFEST_KEYS: &[&str] = &[
    "schema_version",
    "minimum_engine_version",
    "id",
    "name",
    "description",
    "category",
    "preview_path",
    "backdrop",
    "palette",
    "effects",
    "assets",
    "rights",
];
const OPTIONAL_MANIFEST_KEYS: &[&str] = &["adaptation"];

#[derive(Clone, Debug)]
pub struct LocalThemeCatalog {
    root: PathBuf,
}

impl LocalThemeCatalog {
    pub fn in_directory(state_directory: &Path) -> Result<Self, String> {
        let root = state_directory.join(LOCAL_THEME_DIRECTORY);
        if root.exists() && is_link(&root)? {
            return Err("Local theme directory is unavailable".to_owned());
        }
        fs::create_dir_all(&root).map_err(|_| "Local theme directory is unavailable".to_owned())?;
        crate::private_state::protect_owned_path(&root)?;
        Ok(Self { root })
    }

    pub fn packs(&self) -> Vec<ThemePack> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut packs = entries
            .filter_map(Result::ok)
            .filter_map(|entry| self.load_pack(&entry.path()).map(|(pack, _)| pack))
            .collect::<Vec<_>>();
        packs.sort_by(|left, right| left.id.cmp(&right.id));
        packs
    }

    pub fn asset_bytes(&self, theme_id: &str) -> Option<Vec<u8>> {
        let directory = self.root.join(theme_id);
        let (pack, bytes) = self.load_pack(&directory)?;
        (pack.id == theme_id).then_some(bytes)
    }

    pub fn preview_data_url(&self, theme_id: &str) -> Option<String> {
        let directory = self.root.join(theme_id);
        let (pack, bytes) = self.load_pack(&directory)?;
        let asset = pack.assets.first()?;
        Some(format!(
            "data:{};base64,{}",
            asset.mime_type,
            STANDARD.encode(bytes)
        ))
    }

    pub fn import_image(
        &self,
        display_name: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> Result<ThemePack, String> {
        if bytes.is_empty() || bytes.len() > MAX_RUNTIME_THEME_ASSET_BYTES as usize {
            return Err("Local theme image is outside the supported size range".to_owned());
        }
        let (detected_mime, extension) = detect_image(bytes)
            .ok_or_else(|| "Local theme image format is unsupported".to_owned())?;
        if mime_type != detected_mime {
            return Err("Local theme image type does not match its contents".to_owned());
        }

        let hash = format!("{:x}", Sha256::digest(bytes));
        let id = format!("local-{}", &hash[..16]);
        let directory = self.root.join(&id);
        if directory.exists() {
            return self
                .load_pack(&directory)
                .map(|(pack, _)| pack)
                .ok_or_else(|| "Existing local theme is invalid".to_owned());
        }

        let name = safe_import_name(display_name, &id);
        let pack = ThemePack {
            schema_version: 1,
            minimum_engine_version: 1,
            id: id.clone(),
            name,
            description: "仅保存在当前设备上的个人图片主题".to_owned(),
            category: ThemeCategory::LocalImport,
            marketplace: None,
            preview_path: format!("local-theme:{id}"),
            backdrop: ThemeBackdrop::Image {
                asset_id: id.clone(),
                overlay: "#17202b".to_owned(),
                focal_x: 50,
                focal_y: 50,
            },
            palette: ThemePalette {
                surface: "#18212d".to_owned(),
                surface_strong: "#101720".to_owned(),
                text: "#f2f7fb".to_owned(),
                accent: "#7acfea".to_owned(),
                border: "#7f9caf".to_owned(),
            },
            effects: ThemeEffects {
                surface_opacity: 78,
                blur_px: 10,
                contrast_percent: 100,
                motion: false,
            },
            adaptation: Some(analyze_backdrop(bytes)?),
            assets: vec![ThemeAsset {
                id: id.clone(),
                mime_type: detected_mime.to_owned(),
                sha256: hash,
            }],
            rights: ThemeRights {
                source: "User-owned local import".to_owned(),
                rightsholder: "User-provided asset".to_owned(),
                license: "Local use only".to_owned(),
                commercial_redistribution: false,
                attribution: "Stored locally by user request".to_owned(),
                reviewed_at: chrono::Local::now().format("%Y-%m-%d").to_string(),
                manual_signoff: true,
                status: RightsStatus::LocalOnly,
            },
        };
        validate_theme_pack(&pack, false)
            .map_err(|_| "Generated local theme metadata is invalid".to_owned())?;

        let staging = self.root.join(format!(".import-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&staging).map_err(|_| "Local theme could not be imported".to_owned())?;
        let result = (|| {
            crate::private_state::protect_owned_path(&staging)?;
            write_owned_file(&staging.join(format!("{id}.{extension}")), bytes)?;
            let manifest = serde_json::to_vec_pretty(&pack)
                .map_err(|_| "Local theme manifest could not be encoded".to_owned())?;
            write_owned_file(&staging.join(MANIFEST_FILE), &manifest)?;
            fs::rename(&staging, &directory)
                .map_err(|_| "Local theme could not be committed".to_owned())?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        self.load_pack(&directory)
            .map(|(loaded, _)| loaded)
            .ok_or_else(|| "Imported local theme failed validation".to_owned())
    }

    fn load_pack(&self, directory: &Path) -> Option<(ThemePack, Vec<u8>)> {
        if directory.parent() != Some(self.root.as_path())
            || !directory.is_dir()
            || is_link(directory).ok()?
        {
            return None;
        }
        let manifest_path = directory.join(MANIFEST_FILE);
        let manifest_metadata = manifest_path.symlink_metadata().ok()?;
        if !manifest_metadata.is_file()
            || manifest_metadata.file_type().is_symlink()
            || manifest_metadata.len() > MAX_MANIFEST_BYTES
        {
            return None;
        }
        let manifest_bytes = fs::read(&manifest_path).ok()?;
        let raw = serde_json::from_slice::<Value>(&manifest_bytes).ok()?;
        if !has_exact_manifest_keys(&raw) {
            return None;
        }
        let pack = serde_json::from_value::<ThemePack>(raw).ok()?;
        if validate_theme_pack(&pack, false).is_err()
            || pack.category != ThemeCategory::LocalImport
            || pack.preview_path != format!("local-theme:{}", pack.id)
            || pack.rights.status != RightsStatus::LocalOnly
            || pack.rights.commercial_redistribution
            || pack.assets.len() != 1
        {
            return None;
        }
        let ThemeBackdrop::Image { asset_id, .. } = &pack.backdrop else {
            return None;
        };
        let asset = pack.assets.first()?;
        if asset.id != *asset_id || directory.file_name()?.to_str()? != pack.id {
            return None;
        }
        let extension = match asset.mime_type.as_str() {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            _ => return None,
        };
        let asset_path = directory.join(format!("{}.{}", asset.id, extension));
        let metadata = asset_path.symlink_metadata().ok()?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() == 0
            || metadata.len() > MAX_RUNTIME_THEME_ASSET_BYTES
        {
            return None;
        }
        let bytes = fs::read(asset_path).ok()?;
        if format!("{:x}", Sha256::digest(&bytes)) != asset.sha256.to_ascii_lowercase() {
            return None;
        }
        Some((pack, bytes))
    }
}

fn analyze_backdrop(bytes: &[u8]) -> Result<ThemeAdaptation, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|_| "Local theme image cannot be analyzed".to_owned())?
        .thumbnail_exact(64, 64)
        .to_rgb8();
    let width = image.width() as usize;
    let height = image.height() as usize;
    let mut luminance = Vec::with_capacity(width * height);
    let mut luminance_total = 0.0_f64;
    let mut saturation_total = 0.0_f64;

    for pixel in image.pixels() {
        let red = f64::from(pixel[0]) / 255.0;
        let green = f64::from(pixel[1]) / 255.0;
        let blue = f64::from(pixel[2]) / 255.0;
        let maximum = red.max(green).max(blue);
        let minimum = red.min(green).min(blue);
        let value = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
        luminance.push(value);
        luminance_total += value;
        saturation_total += if maximum == 0.0 {
            0.0
        } else {
            (maximum - minimum) / maximum
        };
    }

    let sample_count = luminance.len() as f64;
    let average = luminance_total / sample_count;
    let deviation = (luminance
        .iter()
        .map(|value| (value - average).powi(2))
        .sum::<f64>()
        / sample_count)
        .sqrt();
    let mut edge_total = 0.0_f64;
    let mut edge_count = 0_usize;
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            if x + 1 < width {
                edge_total += (luminance[index] - luminance[index + 1]).abs();
                edge_count += 1;
            }
            if y + 1 < height {
                edge_total += (luminance[index] - luminance[index + width]).abs();
                edge_count += 1;
            }
        }
    }
    let edge_density = edge_total / edge_count.max(1) as f64;
    let percent = |value: f64| value.round().clamp(0.0, 100.0) as u8;

    Ok(ThemeAdaptation {
        luminance: percent(average * 100.0),
        complexity: percent(edge_density * 260.0 + deviation * 95.0),
        saturation: percent(saturation_total / sample_count * 100.0),
    })
}

fn write_owned_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = File::create(path).map_err(|_| "Local theme file could not be created")?;
    crate::private_state::protect_owned_path(path)?;
    file.write_all(bytes)
        .map_err(|_| "Local theme file could not be written".to_owned())?;
    file.sync_all()
        .map_err(|_| "Local theme file could not be synchronized".to_owned())
}

fn detect_image(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else {
        None
    }
}

fn safe_import_name(value: &str, id: &str) -> String {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    let invalid = trimmed.is_empty()
        || trimmed.chars().count() > 80
        || trimmed.chars().any(char::is_control)
        || lower.contains("javascript:")
        || lower.contains("<script")
        || lower.contains("http://")
        || lower.contains("https://");
    if invalid {
        format!("本机主题 {}", &id[6..12])
    } else {
        trimmed.to_owned()
    }
}

fn has_exact_manifest_keys(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    (object.len() == MANIFEST_KEYS.len()
        || object.len() == MANIFEST_KEYS.len() + OPTIONAL_MANIFEST_KEYS.len())
        && object.keys().all(|key| {
            MANIFEST_KEYS.contains(&key.as_str()) || OPTIONAL_MANIFEST_KEYS.contains(&key.as_str())
        })
}

fn is_link(path: &Path) -> Result<bool, String> {
    path.symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .map_err(|_| "Local theme path is unavailable".to_owned())
}
