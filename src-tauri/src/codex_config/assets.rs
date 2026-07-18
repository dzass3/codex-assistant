use sha2::{Digest, Sha256};

pub const ASSET_VERSION: &str = "1";
pub const PROFILE_VERSION: &str = "1";
pub const AGENT_DIRECTORY: &str = "codex-assistant";
pub const SKILL_DIRECTORY: &str = "codex-assistant-smart-routing";

#[derive(Clone, Copy)]
pub(crate) struct Asset {
    pub(crate) label: &'static str,
    pub(crate) relative_path: &'static str,
    pub(crate) bytes: &'static [u8],
    pub(crate) is_skill: bool,
}

pub(crate) const ASSETS: &[Asset] = &[
    Asset {
        label: "spark.toml",
        relative_path: "spark.toml",
        bytes: include_bytes!("../../resources/routing/agents/spark.toml"),
        is_skill: false,
    },
    Asset {
        label: "luna.toml",
        relative_path: "luna.toml",
        bytes: include_bytes!("../../resources/routing/agents/luna.toml"),
        is_skill: false,
    },
    Asset {
        label: "terra.toml",
        relative_path: "terra.toml",
        bytes: include_bytes!("../../resources/routing/agents/terra.toml"),
        is_skill: false,
    },
    Asset {
        label: "sol.toml",
        relative_path: "sol.toml",
        bytes: include_bytes!("../../resources/routing/agents/sol.toml"),
        is_skill: false,
    },
    Asset {
        label: "SKILL.md",
        relative_path: "SKILL.md",
        bytes: include_bytes!("../../resources/routing/skill/SKILL.md"),
        is_skill: true,
    },
    Asset {
        label: "references/policy.md",
        relative_path: "references/policy.md",
        bytes: include_bytes!("../../resources/routing/skill/references/policy.md"),
        is_skill: true,
    },
];

pub(crate) fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}
