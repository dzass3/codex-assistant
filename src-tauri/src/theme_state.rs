use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn prepare_default_theme_state() -> Result<PathBuf, String> {
    let config_root = dirs::config_dir()
        .ok_or_else(|| "Theme state directory is unavailable".to_owned())?
        .join("codex-agent-monitor");
    let legacy_state = config_root.join("routing");
    let theme_state = config_root.join("themes");
    migrate_theme_state(&legacy_state, &theme_state)?;
    Ok(theme_state)
}

pub fn migrate_theme_state(legacy: &Path, destination: &Path) -> Result<(), String> {
    reject_link(destination)?;
    fs::create_dir_all(destination).map_err(|_| "Theme state directory could not be created")?;
    crate::private_state::protect_owned_path(destination)?;
    if !legacy.exists() {
        return Ok(());
    }
    reject_link(legacy)?;
    for name in ["theme-state.json", "control-session.json", "local-themes"] {
        let source = legacy.join(name);
        let target = destination.join(name);
        if !source.exists() || target.exists() {
            continue;
        }
        reject_link(&source)?;
        fs::rename(&source, &target).map_err(|_| "Legacy theme state could not be migrated")?;
        crate::private_state::protect_owned_path(&target)?;
    }
    Ok(())
}

fn reject_link(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "Theme state path metadata could not be read".to_owned())?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_type().is_symlink() || metadata.file_attributes() & 0x0400 != 0 {
            return Err("Theme state path may not be a link or reparse point".to_owned());
        }
    }
    #[cfg(not(windows))]
    if metadata.file_type().is_symlink() {
        return Err("Theme state path may not be a link".to_owned());
    }
    Ok(())
}
