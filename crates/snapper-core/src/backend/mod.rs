pub mod cli;
#[cfg(feature = "dbus")]
pub mod dbus;
pub mod mock;

pub use cli::SnapperCliBackend;
#[cfg(feature = "dbus")]
pub use dbus::SnapperDbusBackend;
pub use mock::MockSnapperBackend;

use crate::model::{
    BatchDeleteReport, CleanupAlgorithm, ConfigInfo, ConfigSetting, CreateSnapshotRequest,
    DiffOutput, FileChange, ModifySnapshotRequest, Snapshot, SnapshotKey,
};
use anyhow::Result;
use std::fmt;

/// Typed error indicating Snapper requires administrator authentication before proceeding.
#[derive(Debug, Clone)]
pub struct AuthenticationRequiredError {
    pub detail: String,
}

impl AuthenticationRequiredError {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl fmt::Display for AuthenticationRequiredError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for AuthenticationRequiredError {}

/// Returns `true` if any cause in `err` is an [`AuthenticationRequiredError`].
pub fn is_authentication_required(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.is::<AuthenticationRequiredError>())
}

/// How CLI invocations handle root privileges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationStrategy {
    /// Invoke `snapper` directly (works as root or when ALLOW_USERS / SYNC_ACL grants access).
    Direct,
    /// Use `sudo -n snapper` when not root so raw terminal mode is never corrupted.
    SudoNonInteractive,
    /// Try `snapper` directly first; fall back to Polkit `pkexec snapper` on permission errors.
    PkexecFallback,
}

/// Abstract Snapper operations backend (CLI, D-Bus, or deterministic Mock).
pub trait SnapperBackend: Send + Sync {
    fn backend_label(&self) -> String;

    fn is_mock(&self) -> bool {
        false
    }

    /// True when the backend must prompt interactively (e.g. Polkit) and should avoid two-phase refreshes.
    fn prefers_single_phase_list(&self) -> bool {
        false
    }

    /// True when this backend supports session-scoped `sudo` password unlock or Polkit elevation.
    fn supports_password_auth(&self) -> bool {
        false
    }

    /// True when an administrator unlock (sudo session or Polkit mode) is currently active.
    fn is_authenticated(&self) -> bool {
        false
    }

    /// Validate and store session-scoped administrator credentials (e.g. via `sudo -S`).
    fn authenticate_with_password(&self, _password: &str) -> Result<()> {
        anyhow::bail!("Password authentication is not supported by this backend")
    }

    /// Switch elevation to Polkit (`pkexec --disable-internal-agent`) if a Polkit agent is active.
    fn enable_pkexec_mode(&self) -> Result<()> {
        anyhow::bail!("Polkit elevation is not supported by this backend")
    }

    /// Clear any in-memory session credentials or Polkit elevation state.
    fn clear_authentication(&self) {}

    fn list_configs(&self) -> Result<Vec<ConfigInfo>>;

    fn list_snapshots(
        &self,
        config: Option<&str>,
        include_used_space: bool,
    ) -> Result<Vec<Snapshot>>;

    fn create_snapshot(&self, req: &CreateSnapshotRequest) -> Result<Option<u32>>;

    fn modify_snapshot(&self, req: &ModifySnapshotRequest) -> Result<()>;

    fn delete_snapshots(&self, targets: &[SnapshotKey]) -> BatchDeleteReport;

    fn rollback_snapshot(&self, key: &SnapshotKey) -> Result<()>;

    fn snapshot_status(&self, config: &str, from: u32, to: u32) -> Result<Vec<FileChange>>;

    fn snapshot_diff(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
        max_bytes: usize,
    ) -> Result<DiffOutput>;

    fn undo_changes(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
    ) -> Result<String>;

    fn get_config(&self, config: &str) -> Result<Vec<ConfigSetting>>;

    fn set_config(&self, config: &str, settings: &[ConfigSetting]) -> Result<()>;

    fn run_cleanup(&self, config: &str, algorithm: CleanupAlgorithm) -> Result<()>;
}
