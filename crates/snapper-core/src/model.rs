use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;

fn deserialize_opt_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OptU32 {
        Int(u32),
        Str(String),
    }

    match Option::<OptU32>::deserialize(deserializer)? {
        None => Ok(None),
        Some(OptU32::Int(v)) => Ok(Some(v)),
        Some(OptU32::Str(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<u32>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
    }
}

fn deserialize_opt_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OptU64 {
        Int(u64),
        Str(String),
    }

    match Option::<OptU64>::deserialize(deserializer)? {
        None => Ok(None),
        Some(OptU64::Int(v)) => Ok(Some(v)),
        Some(OptU64::Str(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<u64>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
    }
}

fn deserialize_opt_non_empty_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        None => Ok(None),
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
    }
}

/// Unique domain identifier for a Snapper snapshot across configurations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SnapshotKey {
    pub config: String,
    pub number: u32,
}

impl SnapshotKey {
    pub fn new(config: impl Into<String>, number: u32) -> Self {
        Self {
            config: config.into(),
            number,
        }
    }
}

impl fmt::Display for SnapshotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.config.is_empty() {
            write!(f, "#{}", self.number)
        } else {
            write!(f, "{}:#{}", self.config, self.number)
        }
    }
}

/// Snapshot entry parsed from `snapper --jsonout list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub config: String,
    #[serde(default)]
    pub subvolume: String,
    #[serde(default)]
    pub number: u32,
    #[serde(rename = "type", default)]
    pub snapshot_type: String,
    #[serde(rename = "pre-number", default, deserialize_with = "deserialize_opt_u32")]
    pub pre_number: Option<u32>,
    #[serde(rename = "post-number", default, deserialize_with = "deserialize_opt_u32")]
    pub post_number: Option<u32>,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub user: String,
    #[serde(default, deserialize_with = "deserialize_opt_non_empty_string")]
    pub cleanup: Option<String>,
    #[serde(default)]
    pub description: String,
    pub userdata: Option<BTreeMap<String, String>>,
    #[serde(rename = "used-space", default, deserialize_with = "deserialize_opt_u64")]
    pub used_space: Option<u64>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub active: bool,
}

impl Snapshot {
    /// Returns the domain key `(config, number)` for this snapshot.
    pub fn key(&self) -> SnapshotKey {
        SnapshotKey::new(self.config.clone(), self.number)
    }

    /// Snapshots #0 (live subvolume), active (currently booted), or default (next boot) cannot be deleted.
    pub fn is_protected(&self) -> bool {
        self.number == 0 || self.active || self.default
    }

    /// True if `userdata["important"]` marks this snapshot as important.
    pub fn is_important(&self) -> bool {
        self.userdata
            .as_ref()
            .and_then(|u| u.get("important"))
            .is_some_and(|v| {
                v.eq_ignore_ascii_case("yes") || v.eq_ignore_ascii_case("true") || v == "1"
            })
    }

    /// True if no automatic cleanup algorithm is attached (excluding #0).
    pub fn is_pinned(&self) -> bool {
        self.number != 0 && self.cleanup.as_deref().is_none_or(|c| c.trim().is_empty())
    }

    pub fn is_pre(&self) -> bool {
        self.snapshot_type == "pre"
    }

    pub fn is_post(&self) -> bool {
        self.snapshot_type == "post"
    }
}

/// A Snapper configuration reported by `snapper list-configs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigInfo {
    pub name: String,
    pub subvolume: String,
}

/// Key-value configuration option reported by `snapper get-config`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSetting {
    pub key: String,
    pub value: String,
}

impl ConfigSetting {
    /// `SUBVOLUME` and `FSTYPE` cannot be modified via `snapper set-config`.
    pub fn is_readonly(&self) -> bool {
        matches!(self.key.as_str(), "SUBVOLUME" | "FSTYPE")
    }
}

/// Snapshot creation type for `snapper create`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotCreateType {
    #[default]
    Single,
    Pre,
    Post { pre_number: u32 },
}

impl SnapshotCreateType {
    pub fn as_cli_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Pre => "pre",
            Self::Post { .. } => "post",
        }
    }
}

/// Snapper cleanup algorithm identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupAlgorithm {
    Number,
    Timeline,
    EmptyPrePost,
    All,
    Custom(String),
}

impl CleanupAlgorithm {
    pub fn as_cli_str(&self) -> &str {
        match self {
            Self::Number => "number",
            Self::Timeline => "timeline",
            Self::EmptyPrePost => "empty-pre-post",
            Self::All => "all",
            Self::Custom(s) => s.as_str(),
        }
    }

    pub fn from_cli_str(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(match trimmed {
            "number" => Self::Number,
            "timeline" => Self::Timeline,
            "empty-pre-post" => Self::EmptyPrePost,
            "all" => Self::All,
            other => Self::Custom(other.to_string()),
        })
    }
}

impl fmt::Display for CleanupAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_cli_str())
    }
}

/// Request payload for `snapper create`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSnapshotRequest {
    pub config: String,
    pub description: String,
    pub snapshot_type: SnapshotCreateType,
    pub cleanup: Option<CleanupAlgorithm>,
    pub userdata: BTreeMap<String, String>,
}

impl CreateSnapshotRequest {
    pub fn single(config: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            config: config.into(),
            description: description.into(),
            snapshot_type: SnapshotCreateType::Single,
            cleanup: None,
            userdata: BTreeMap::new(),
        }
    }
}

/// Request payload for `snapper modify`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifySnapshotRequest {
    pub key: SnapshotKey,
    pub description: Option<String>,
    /// `Some(Some(alg))` sets algorithm, `Some(None)` unsets (pins), `None` leaves unchanged.
    pub cleanup: Option<Option<CleanupAlgorithm>>,
    pub userdata: Option<BTreeMap<String, String>>,
}

/// Primary file content status from `snapper status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentChange {
    Unchanged,
    Created,
    Deleted,
    Modified,
}

/// Parsed 6-character Snapper status flags (`[+|-|c|.][p|.][u|.][g|.][x|.][a|.]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStatusFlags {
    pub content: ContentChange,
    pub permissions: bool,
    pub owner: bool,
    pub group: bool,
    pub xattr: bool,
    pub acl: bool,
}

impl FileStatusFlags {
    pub fn summary_badge(&self) -> &'static str {
        match self.content {
            ContentChange::Created => "A",
            ContentChange::Deleted => "D",
            ContentChange::Modified => "M",
            ContentChange::Unchanged => "Attr",
        }
    }

    pub fn has_metadata_changes(&self) -> bool {
        self.permissions || self.owner || self.group || self.xattr || self.acl
    }

    pub fn to_flag_string(&self) -> String {
        let mut s = String::with_capacity(6);
        s.push(match self.content {
            ContentChange::Created => '+',
            ContentChange::Deleted => '-',
            ContentChange::Modified => 'c',
            ContentChange::Unchanged => '.',
        });
        s.push(if self.permissions { 'p' } else { '.' });
        s.push(if self.owner { 'u' } else { '.' });
        s.push(if self.group { 'g' } else { '.' });
        s.push(if self.xattr { 'x' } else { '.' });
        s.push(if self.acl { 'a' } else { '.' });
        s
    }
}

/// Single file entry returned by `snapper status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub flags: FileStatusFlags,
}

/// Hierarchical folder/file node built from flat `FileChange` entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeNode {
    pub id: String,
    pub label: String,
    pub full_path: String,
    pub change: Option<FileChange>,
    pub children: Vec<FileTreeNode>,
}

/// Unified diff output returned by `snapper diff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffOutput {
    pub patch: String,
    pub truncated: bool,
    pub total_bytes: usize,
}

/// Result of a batch delete operation preserving per-snapshot failure reasons.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BatchDeleteReport {
    pub deleted: Vec<SnapshotKey>,
    pub failed: Vec<(SnapshotKey, String)>,
}

impl BatchDeleteReport {
    pub fn success_count(&self) -> usize {
        self.deleted.len()
    }

    pub fn fail_count(&self) -> usize {
        self.failed.len()
    }
}
