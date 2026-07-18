use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use toml_edit::{value, Array, DocumentMut, Item, Table, Value};
use uuid::Uuid;

use super::assets::{
    sha256, Asset, AGENT_DIRECTORY, ASSETS, ASSET_VERSION, PROFILE_VERSION, SKILL_DIRECTORY,
};

const BACKUP_DIRECTORY: &str = "codex-assistant-backups";
const JOURNAL_DIRECTORY: &str = "codex-assistant-journal";
const MANIFEST_FILE: &str = "manifest.json";
const CONFIG_FILE: &str = "config.toml";
const OWNED_AGENT_NAMES: &[(&str, &str, &str)] = &[
    (
        "codex_assistant_spark",
        "Mechanical, fully specified, low-risk native work",
        "spark.toml",
    ),
    (
        "codex_assistant_luna",
        "Bounded, low-risk native implementation work",
        "luna.toml",
    ),
    (
        "codex_assistant_terra",
        "Cross-layer native implementation and independent review",
        "terra.toml",
    ),
    (
        "codex_assistant_sol",
        "High-risk, architectural, and escalation native work",
        "sol.toml",
    ),
];
const ENABLED_TOOLS: &[&str] = &[
    "routing_policy_get",
    "routing_route_started",
    "routing_quality_record",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallRequest {
    pub codex_home: PathBuf,
    pub global_skill_root: PathBuf,
    pub current_executable: PathBuf,
    pub operation_id: String,
    pub failure_point: Option<FailurePoint>,
}

impl InstallRequest {
    pub fn new(
        codex_home: PathBuf,
        global_skill_root: PathBuf,
        current_executable: PathBuf,
    ) -> Self {
        Self {
            codex_home,
            global_skill_root,
            current_executable,
            operation_id: Uuid::new_v4().to_string(),
            failure_point: None,
        }
    }

    pub fn with_operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = operation_id.into();
        self
    }

    pub fn fail_at(mut self, point: FailurePoint) -> Self {
        self.failure_point = Some(point);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailurePoint {
    Backup,
    Journal,
    AssetStaging,
    ConfigParse,
    TempSync,
    ReplaceAsset,
    ReplaceConfig,
    PostWriteValidation,
    CommitMark,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexConfigService {
    request: InstallRequest,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstallReceipt {
    pub changed: bool,
    pub conflicts: Vec<String>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InspectReceipt {
    pub installed: bool,
    pub recovered_incomplete_operation: bool,
    pub conflicts: Vec<String>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RestoreReceipt {
    pub changed: bool,
    pub conflicts: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError(&'static str);
impl ConfigError {
    fn new(message: &'static str) -> Self {
        Self(message)
    }
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for ConfigError {}
type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Serialize, Deserialize, Clone)]
struct Preimage {
    label: String,
    path: PathBuf,
    existed: bool,
    bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    operation_id: String,
    committed: bool,
    recovered: bool,
    preimages: Vec<Preimage>,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    asset_version: String,
    profile_version: String,
    backup_operation_id: String,
    files: BTreeMap<String, ManifestFile>,
}
#[derive(Serialize, Deserialize)]
struct ManifestFile {
    relative_destination: String,
    installed_hash: String,
    existed_before_install: bool,
}

impl CodexConfigService {
    pub fn new(request: InstallRequest) -> Result<Self> {
        validate_request(&request)?;
        Ok(Self { request })
    }

    pub fn inspect(&self) -> Result<InspectReceipt> {
        let recovered = self.recover_incomplete()?;
        let manifest = self.read_manifest()?;
        let mut conflicts = Vec::new();
        if let Some(manifest) = &manifest {
            for (label, entry) in &manifest.files {
                let path = self.destination_for(&entry.relative_destination)?;
                if path.exists() && !hash_matches(&path, &entry.installed_hash)? {
                    conflicts.push(label.clone());
                }
            }
        }
        Ok(InspectReceipt {
            installed: manifest.is_some(),
            recovered_incomplete_operation: recovered,
            conflicts,
        })
    }

    pub fn install(&self) -> Result<InstallReceipt> {
        self.recover_incomplete()?;
        validate_request(&self.request)?;
        self.ensure_roots()?;
        let targets = self.owned_targets()?;
        let preimages = targets
            .iter()
            .map(|(label, path)| capture(label, path))
            .collect::<Result<Vec<_>>>()?;
        let backup = self.backup_path();
        write_private_json(&backup, &preimages)?;
        self.inject(FailurePoint::Backup)?;
        let journal_path = self.journal_path();
        let mut journal = Journal {
            operation_id: self.request.operation_id.clone(),
            committed: false,
            recovered: false,
            preimages,
        };
        write_private_json(&journal_path, &journal)?;
        self.inject(FailurePoint::Journal)?;

        let result = (|| {
            let changed = self.install_after_journal(&targets)?;
            self.inject(FailurePoint::CommitMark)?;
            journal.committed = true;
            write_private_json(&journal_path, &journal)?;
            Ok(changed)
        })();
        match result {
            Ok(changed) => Ok(InstallReceipt {
                changed,
                conflicts: Vec::new(),
            }),
            Err(error) => {
                restore_preimages(&journal.preimages)?;
                cleanup_staged(&targets, &self.request.operation_id);
                Err(error)
            }
        }
    }

    pub fn restore(&self) -> Result<RestoreReceipt> {
        self.recover_incomplete()?;
        let Some(manifest) = self.read_manifest()? else {
            return Ok(RestoreReceipt::default());
        };
        let backup = self.read_backup(&manifest.backup_operation_id)?;
        self.validate_journal_preimages(&backup)?;
        let mut conflicts = Vec::new();
        let mut changed = false;
        for (label, entry) in &manifest.files {
            let path = self.destination_for(&entry.relative_destination)?;
            reject_link_components(&path)?;
            if !path.exists() {
                continue;
            }
            if hash_matches(&path, &entry.installed_hash)? {
                let original = backup
                    .iter()
                    .find(|image| image.label == *label)
                    .ok_or_else(|| ConfigError::new("Routing backup is incomplete"))?;
                restore_preimages(std::slice::from_ref(original))?;
                changed = true;
            } else {
                conflicts.push(label.clone());
            }
        }
        let manifest_path = self.agent_root().join(MANIFEST_FILE);
        if manifest_path.exists() && !conflicts.iter().any(|item| item == MANIFEST_FILE) {
            fs::remove_file(manifest_path)
                .map_err(|_| ConfigError::new("Owned routing manifest could not be removed"))?;
            changed = true;
        }
        Ok(RestoreReceipt { changed, conflicts })
    }

    fn install_after_journal(&self, targets: &[(String, PathBuf)]) -> Result<bool> {
        let config_path = self.config_path();
        let original = if config_path.exists() {
            read_bytes(&config_path)?
        } else {
            Vec::new()
        };
        let config = self.merge_config(&original)?;
        let changed = original != config.as_bytes()
            || ASSETS.iter().any(|asset| {
                self.asset_destination(asset).ok().is_some_and(|path| {
                    read_bytes(&path).map_or(true, |actual| actual != asset.bytes)
                })
            });
        self.inject(FailurePoint::ConfigParse)?;
        let mut staged = Vec::new();
        for asset in ASSETS {
            let destination = self.asset_destination(asset)?;
            stage(
                &destination,
                asset.bytes,
                &self.request.operation_id,
                self.request.failure_point == Some(FailurePoint::TempSync),
            )?;
            staged.push((destination, asset.bytes));
        }
        let manifest_bytes = serde_json::to_vec_pretty(&self.manifest_for_targets(targets)?)
            .map_err(|_| ConfigError::new("Routing manifest could not be encoded"))?;
        let manifest_path = self.agent_root().join(MANIFEST_FILE);
        stage(
            &manifest_path,
            &manifest_bytes,
            &self.request.operation_id,
            self.request.failure_point == Some(FailurePoint::TempSync),
        )?;
        staged.push((manifest_path, &manifest_bytes));
        self.inject(FailurePoint::AssetStaging)?;
        for (destination, _) in &staged {
            self.inject(FailurePoint::ReplaceAsset)?;
            replace_staged(destination, &self.request.operation_id)?;
        }
        stage(
            &config_path,
            config.as_bytes(),
            &self.request.operation_id,
            self.request.failure_point == Some(FailurePoint::TempSync),
        )?;
        self.inject(FailurePoint::ReplaceConfig)?;
        replace_staged(&config_path, &self.request.operation_id)?;
        self.validate_installed()?;
        self.inject(FailurePoint::PostWriteValidation)?;
        Ok(changed)
    }

    fn merge_config(&self, original: &[u8]) -> Result<String> {
        if original.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return Err(ConfigError::new(
                "Codex config has an unsupported byte-order mark",
            ));
        }
        let source = if original.is_empty() {
            ""
        } else {
            std::str::from_utf8(original)
                .map_err(|_| ConfigError::new("Codex config is not UTF-8"))?
        };
        let mut document = source
            .parse::<DocumentMut>()
            .map_err(|_| ConfigError::new("Codex config is malformed"))?;
        if document.as_table().contains_key("agents") && !document["agents"].is_table() {
            return Err(ConfigError::new("Codex agents setting has the wrong type"));
        }
        if !document.as_table().contains_key("agents") {
            document["agents"] = Item::Table(Table::new());
        }
        let agents = document["agents"]
            .as_table_mut()
            .ok_or_else(|| ConfigError::new("Codex agents setting has the wrong type"))?;
        match agents.get("max_depth") {
            Some(Item::Value(Value::Integer(depth))) if *depth.value() >= 2 => {}
            Some(Item::Value(Value::Integer(_))) | None => {
                agents["max_depth"] = value(2);
            }
            _ => return Err(ConfigError::new("Codex max depth has the wrong type")),
        }
        for (name, description, filename) in OWNED_AGENT_NAMES {
            let desired = self
                .agent_root()
                .join(filename)
                .to_string_lossy()
                .into_owned();
            if agents.contains_key(name) && !agents[name].is_table() {
                return Err(ConfigError::new(
                    "Owned Codex agent setting has the wrong type",
                ));
            }
            if !agents.contains_key(name) {
                agents[name] = Item::Table(Table::new());
            }
            let table = agents[name]
                .as_table_mut()
                .ok_or_else(|| ConfigError::new("Owned Codex agent setting has the wrong type"))?;
            set_or_verify_string(table, "description", description)?;
            set_or_verify_string(table, "config_file", &desired)?;
        }
        if document.as_table().contains_key("mcp_servers") && !document["mcp_servers"].is_table() {
            return Err(ConfigError::new("Codex MCP setting has the wrong type"));
        }
        if !document.as_table().contains_key("mcp_servers") {
            document["mcp_servers"] = Item::Table(Table::new());
        }
        let servers = document["mcp_servers"]
            .as_table_mut()
            .ok_or_else(|| ConfigError::new("Codex MCP setting has the wrong type"))?;
        if servers.contains_key("codex_assistant_routing")
            && !servers["codex_assistant_routing"].is_table()
        {
            return Err(ConfigError::new(
                "Owned Codex MCP setting has the wrong type",
            ));
        }
        if !servers.contains_key("codex_assistant_routing") {
            servers["codex_assistant_routing"] = Item::Table(Table::new());
        }
        let mcp = servers["codex_assistant_routing"]
            .as_table_mut()
            .ok_or_else(|| ConfigError::new("Owned Codex MCP setting has the wrong type"))?;
        set_or_verify_string(
            mcp,
            "command",
            &self.request.current_executable.to_string_lossy(),
        )?;
        set_or_verify_array(mcp, "args", &["routing-mcp"])?;
        set_or_verify_bool(mcp, "enabled", true)?;
        set_or_verify_bool(mcp, "required", false)?;
        set_or_verify_array(mcp, "enabled_tools", ENABLED_TOOLS)?;
        let rendered = document.to_string();
        Ok(if source.contains("\r\n") {
            rendered.replace("\n", "\r\n")
        } else {
            rendered
        })
    }

    fn validate_installed(&self) -> Result<()> {
        let config = read_bytes(&self.config_path())?;
        self.merge_config(&config)?;
        for asset in ASSETS {
            let bytes = read_bytes(&self.asset_destination(asset)?)?;
            let source = std::str::from_utf8(&bytes)
                .map_err(|_| ConfigError::new("Bundled routing asset is not UTF-8"))?;
            if asset.relative_path.ends_with(".toml") {
                source
                    .parse::<DocumentMut>()
                    .map_err(|_| ConfigError::new("Bundled routing profile is malformed"))?;
            }
            if asset.relative_path == "SKILL.md" && !source.starts_with("---\n") {
                return Err(ConfigError::new("Bundled routing skill is malformed"));
            }
        }
        Ok(())
    }

    fn recover_incomplete(&self) -> Result<bool> {
        let directory = self.journal_directory();
        if !directory.exists() {
            return Ok(false);
        }
        let mut recovered = false;
        for entry in fs::read_dir(&directory)
            .map_err(|_| ConfigError::new("Routing journal could not be inspected"))?
        {
            let path = entry
                .map_err(|_| ConfigError::new("Routing journal could not be inspected"))?
                .path();
            if !path.is_file() {
                continue;
            }
            let mut journal: Journal = serde_json::from_slice(&read_bytes(&path)?)
                .map_err(|_| ConfigError::new("Routing journal is malformed"))?;
            if !journal.committed && !journal.recovered {
                self.validate_journal_preimages(&journal.preimages)?;
                restore_preimages(&journal.preimages)?;
                journal.recovered = true;
                write_private_json(&path, &journal)?;
                recovered = true;
            }
        }
        Ok(recovered)
    }

    fn read_manifest(&self) -> Result<Option<Manifest>> {
        let path = self.agent_root().join(MANIFEST_FILE);
        if !path.exists() {
            return Ok(None);
        }
        serde_json::from_slice(&read_bytes(&path)?)
            .map(Some)
            .map_err(|_| ConfigError::new("Routing ownership manifest is malformed"))
    }
    fn read_backup(&self, operation_id: &str) -> Result<Vec<Preimage>> {
        if operation_id.is_empty() || operation_id.contains(['/', '\\']) {
            return Err(ConfigError::new("Routing backup is invalid"));
        }
        let path = self.backup_directory().join(format!("{operation_id}.json"));
        reject_link_components(&path)?;
        serde_json::from_slice(&read_bytes(&path)?)
            .map_err(|_| ConfigError::new("Routing backup is malformed"))
    }
    fn validate_journal_preimages(&self, preimages: &[Preimage]) -> Result<()> {
        let owned = self.owned_targets()?;
        for image in preimages {
            if !owned
                .iter()
                .any(|(label, path)| label == &image.label && path == &image.path)
            {
                return Err(ConfigError::new(
                    "Routing journal refers outside owned destinations",
                ));
            }
            reject_link_components(&image.path)?;
        }
        Ok(())
    }
    fn manifest_for_targets(&self, targets: &[(String, PathBuf)]) -> Result<Manifest> {
        let existing = self.read_manifest()?;
        let mut files = BTreeMap::new();
        for (label, path) in targets {
            if label == MANIFEST_FILE {
                continue;
            }
            let relative = self.relative_label(path)?;
            let bytes = if label == CONFIG_FILE {
                self.merge_config(&if path.exists() {
                    read_bytes(path)?
                } else {
                    Vec::new()
                })?
                .into_bytes()
            } else {
                ASSETS
                    .iter()
                    .find(|asset| asset.label == label)
                    .map(|asset| asset.bytes.to_vec())
                    .ok_or_else(|| ConfigError::new("Unknown routing asset"))?
            };
            let existed_before_install = existing
                .as_ref()
                .and_then(|manifest| manifest.files.get(label))
                .map_or_else(|| path.exists(), |entry| entry.existed_before_install);
            files.insert(
                label.clone(),
                ManifestFile {
                    relative_destination: relative,
                    installed_hash: sha256(&bytes),
                    existed_before_install,
                },
            );
        }
        Ok(Manifest {
            asset_version: ASSET_VERSION.into(),
            profile_version: PROFILE_VERSION.into(),
            backup_operation_id: existing.as_ref().map_or_else(
                || self.request.operation_id.clone(),
                |manifest| manifest.backup_operation_id.clone(),
            ),
            files,
        })
    }
    fn owned_targets(&self) -> Result<Vec<(String, PathBuf)>> {
        let mut targets = vec![(CONFIG_FILE.into(), self.config_path())];
        for asset in ASSETS {
            targets.push((asset.label.into(), self.asset_destination(asset)?));
        }
        targets.push((MANIFEST_FILE.into(), self.agent_root().join(MANIFEST_FILE)));
        Ok(targets)
    }
    fn asset_destination(&self, asset: &Asset) -> Result<PathBuf> {
        let root = if asset.is_skill {
            self.skill_root()
        } else {
            self.agent_root()
        };
        let path = root.join(asset.relative_path);
        ensure_descendant(&root, &path)?;
        reject_link_components(&path)?;
        Ok(path)
    }
    fn destination_for(&self, relative: &str) -> Result<PathBuf> {
        let (root, rest) = relative
            .split_once(':')
            .ok_or_else(|| ConfigError::new("Routing manifest is invalid"))?;
        let base = match root {
            "agent" => self.agent_root(),
            "skill" => self.skill_root(),
            "config" if rest == CONFIG_FILE => self.request.codex_home.clone(),
            _ => return Err(ConfigError::new("Routing manifest is invalid")),
        };
        let path = base.join(rest);
        ensure_descendant(&base, &path)?;
        reject_link_components(&path)?;
        Ok(path)
    }
    fn relative_label(&self, path: &Path) -> Result<String> {
        if path == self.config_path() {
            return Ok(format!("config:{CONFIG_FILE}"));
        }
        if let Ok(value) = path.strip_prefix(self.agent_root()) {
            return Ok(format!(
                "agent:{}",
                value.to_string_lossy().replace('\\', "/")
            ));
        }
        if let Ok(value) = path.strip_prefix(self.skill_root()) {
            return Ok(format!(
                "skill:{}",
                value.to_string_lossy().replace('\\', "/")
            ));
        }
        Err(ConfigError::new(
            "Routing destination is outside owned roots",
        ))
    }
    fn ensure_roots(&self) -> Result<()> {
        ensure_private_directory(&self.request.codex_home)?;
        ensure_private_directory(&self.agent_root())?;
        ensure_private_directory(&self.skill_root())?;
        ensure_private_directory(&self.skill_root().join("references"))?;
        ensure_private_directory(&self.backup_directory())?;
        ensure_private_directory(&self.journal_directory())
    }
    fn config_path(&self) -> PathBuf {
        self.request.codex_home.join(CONFIG_FILE)
    }
    fn agent_root(&self) -> PathBuf {
        self.request.codex_home.join("agents").join(AGENT_DIRECTORY)
    }
    fn skill_root(&self) -> PathBuf {
        self.request.global_skill_root.join(SKILL_DIRECTORY)
    }
    fn backup_directory(&self) -> PathBuf {
        self.request.codex_home.join(BACKUP_DIRECTORY)
    }
    fn journal_directory(&self) -> PathBuf {
        self.request.codex_home.join(JOURNAL_DIRECTORY)
    }
    fn backup_path(&self) -> PathBuf {
        self.backup_directory()
            .join(format!("{}.json", self.request.operation_id))
    }
    fn journal_path(&self) -> PathBuf {
        self.journal_directory()
            .join(format!("{}.json", self.request.operation_id))
    }
    fn inject(&self, point: FailurePoint) -> Result<()> {
        if self.request.failure_point == Some(point) {
            Err(ConfigError::new("Injected routing transaction failure"))
        } else {
            Ok(())
        }
    }
}

fn validate_request(request: &InstallRequest) -> Result<()> {
    for path in [
        &request.codex_home,
        &request.global_skill_root,
        &request.current_executable,
    ] {
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(ConfigError::new("Routing paths must be absolute"));
        }
    }
    if request.operation_id.is_empty() || request.operation_id.contains(['/', '\\']) {
        return Err(ConfigError::new("Routing operation identifier is invalid"));
    }
    if !request.current_executable.is_file() || is_link(&request.current_executable)? {
        return Err(ConfigError::new(
            "Routing executable must be an absolute regular file",
        ));
    }
    if request.codex_home.exists() && !request.codex_home.is_dir() {
        return Err(ConfigError::new("Codex home is not a directory"));
    }
    let config = request.codex_home.join(CONFIG_FILE);
    if config.exists() && (!config.is_file() || is_link(&config)?) {
        return Err(ConfigError::new("Codex config is not a regular file"));
    }
    reject_link_components(&request.codex_home)?;
    reject_link_components(&request.global_skill_root)?;
    Ok(())
}

fn set_or_verify_string(table: &mut Table, key: &str, desired: &str) -> Result<()> {
    if let Some(item) = table.get(key) {
        if item.as_str() != Some(desired) {
            return Err(ConfigError::new(
                "Owned Codex setting conflicts with another owner",
            ));
        }
    } else {
        table[key] = value(desired);
    }
    Ok(())
}
fn set_or_verify_bool(table: &mut Table, key: &str, desired: bool) -> Result<()> {
    if let Some(item) = table.get(key) {
        if item.as_bool() != Some(desired) {
            return Err(ConfigError::new(
                "Owned Codex setting conflicts with another owner",
            ));
        }
    } else {
        table[key] = value(desired);
    }
    Ok(())
}
fn set_or_verify_array(table: &mut Table, key: &str, desired: &[&str]) -> Result<()> {
    if let Some(item) = table.get(key) {
        let actual = item
            .as_array()
            .ok_or_else(|| ConfigError::new("Owned Codex setting has the wrong type"))?;
        if actual.iter().filter_map(Value::as_str).collect::<Vec<_>>() != desired {
            return Err(ConfigError::new(
                "Owned Codex setting conflicts with another owner",
            ));
        }
    } else {
        let mut array = Array::new();
        for value in desired {
            array.push(*value);
        }
        table[key] = Item::Value(Value::Array(array));
    }
    Ok(())
}

fn capture(label: &str, path: &Path) -> Result<Preimage> {
    Ok(Preimage {
        label: label.into(),
        path: path.into(),
        existed: path.exists(),
        bytes: if path.exists() {
            read_bytes(path)?
        } else {
            Vec::new()
        },
    })
}
fn restore_preimages(preimages: &[Preimage]) -> Result<()> {
    for image in preimages {
        if image.existed {
            if let Some(parent) = image.path.parent() {
                ensure_private_directory(parent)?;
            }
            write_exact(&image.path, &image.bytes)?;
        } else if image.path.exists() {
            fs::remove_file(&image.path)
                .map_err(|_| ConfigError::new("Routing rollback could not remove a new file"))?;
        }
    }
    Ok(())
}
fn stage(
    destination: &Path,
    bytes: &[u8],
    operation: &str,
    inject_sync_failure: bool,
) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| ConfigError::new("Routing destination is invalid"))?;
    ensure_private_directory(parent)?;
    let temporary = temp_path(destination, operation)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| ConfigError::new("Routing asset could not be staged"))?;
    file.write_all(bytes)
        .map_err(|_| ConfigError::new("Routing asset could not be staged"))?;
    if inject_sync_failure {
        return Err(ConfigError::new("Injected routing transaction failure"));
    }
    file.sync_all()
        .map_err(|_| ConfigError::new("Routing asset could not be synced"))?;
    protect_private(&temporary)?;
    Ok(())
}
fn replace_staged(destination: &Path, operation: &str) -> Result<()> {
    let temporary = temp_path(destination, operation)?;
    #[cfg(windows)]
    {
        crate::routing::state::replace_existing(&temporary, destination)
            .map_err(|_| ConfigError::new("Routing asset could not be atomically replaced"))?;
    }
    #[cfg(not(windows))]
    {
        fs::rename(&temporary, destination)
            .map_err(|_| ConfigError::new("Routing asset could not be atomically replaced"))?;
    }
    protect_private(destination)
}
fn write_exact(path: &Path, bytes: &[u8]) -> Result<()> {
    let operation = Uuid::new_v4().to_string();
    stage(path, bytes, &operation, false)?;
    replace_staged(path, &operation)
        .map_err(|_| ConfigError::new("Routing rollback could not replace a file"))
}
fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| ConfigError::new("Routing evidence could not be encoded"))?;
    write_bytes_direct(path, &bytes)
}
fn write_bytes_direct(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_directory(parent)?;
    }
    let mut file = File::create(path)
        .map_err(|_| ConfigError::new("Routing evidence could not be written"))?;
    file.write_all(bytes)
        .map_err(|_| ConfigError::new("Routing evidence could not be written"))?;
    file.sync_all()
        .map_err(|_| ConfigError::new("Routing evidence could not be synced"))?;
    protect_private(path)
}
fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|_| ConfigError::new("Routing file could not be read"))
}
fn hash_matches(path: &Path, hash: &str) -> Result<bool> {
    Ok(sha256(&read_bytes(path)?) == hash)
}
fn temp_path(destination: &Path, operation: &str) -> Result<PathBuf> {
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ConfigError::new("Routing destination is invalid"))?;
    Ok(destination.with_file_name(format!(".{name}.{operation}.tmp")))
}
fn cleanup_staged(targets: &[(String, PathBuf)], operation: &str) {
    for (_, target) in targets {
        if let Ok(temporary) = temp_path(target, operation) {
            let _ = fs::remove_file(temporary);
        }
    }
}
fn ensure_descendant(root: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(root)
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        Err(ConfigError::new(
            "Routing destination is outside owned roots",
        ))
    } else {
        Ok(())
    }
}
fn is_link(path: &Path) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ConfigError::new("Routing path metadata could not be read"))?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        Ok(metadata.file_type().is_symlink() || metadata.file_attributes() & 0x0400 != 0)
    }
    #[cfg(not(windows))]
    {
        Ok(metadata.file_type().is_symlink())
    }
}
fn reject_link_components(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if current.exists() && is_link(&current)? {
            return Err(ConfigError::new(
                "Routing paths may not traverse links or reparse points",
            ));
        }
    }
    Ok(())
}
fn ensure_private_directory(path: &Path) -> Result<()> {
    reject_link_components(path)?;
    fs::create_dir_all(path)
        .map_err(|_| ConfigError::new("Routing directory could not be created"))?;
    protect_private(path)
}
#[cfg(not(windows))]
fn protect_private(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if path.is_dir() { 0o700 } else { 0o600 }),
    )
    .map_err(|_| ConfigError::new("Routing permissions could not be set"))
}
#[cfg(windows)]
fn protect_private(path: &Path) -> Result<()> {
    crate::routing::state::protect_owned_path(path)
        .map_err(|_| ConfigError::new("Routing permissions could not be set"))
}
