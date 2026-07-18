//! Ownership-scoped installation of the native Codex routing assets.
//!
//! This module deliberately has no Tauri command. A later opt-in UI layer owns
//! invoking it; keeping this seam pure makes its filesystem boundary testable.

mod assets;
mod transaction;

pub use assets::{ASSET_VERSION, PROFILE_VERSION};
pub use transaction::{
    CodexConfigService, ConfigError, FailurePoint, InspectReceipt, InstallReceipt, InstallRequest,
    RestoreReceipt,
};
