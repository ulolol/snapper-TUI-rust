use crate::backend::SnapperBackend;
use crate::model::{
    BatchDeleteReport, CleanupAlgorithm, ConfigInfo, ConfigSetting, ContentChange,
    CreateSnapshotRequest, DiffOutput, FileChange, FileStatusFlags, ModifySnapshotRequest,
    Snapshot, SnapshotKey,
};
use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

struct MockState {
    configs: Vec<ConfigInfo>,
    snapshots: Vec<Snapshot>,
    settings: HashMap<String, Vec<ConfigSetting>>,
}

/// Deterministic in-memory Snapper backend for macOS development, demos, and automated UI tests.
pub struct MockSnapperBackend {
    state: Mutex<MockState>,
}

impl MockSnapperBackend {
    pub fn seeded() -> Self {
        let configs = vec![
            ConfigInfo {
                name: "root".to_string(),
                subvolume: "/".to_string(),
            },
            ConfigInfo {
                name: "home".to_string(),
                subvolume: "/home".to_string(),
            },
            ConfigInfo {
                name: "var-log".to_string(),
                subvolume: "/var/log".to_string(),
            },
        ];

        let snapshots = vec![
            make_snap("root", 0, "single", None, None, "", "root", None, "current", None, None, false, false),
            make_snap("root", 145, "post", Some(144), None, "2026-11-21 09:42:18", "root", Some("number"), "pacman -Syu linux 6.12.4", Some(46_800_000), None, true, true),
            make_snap("root", 144, "pre", None, Some(145), "2026-11-21 09:40:02", "root", Some("number"), "pacman -Syu linux 6.12.4", Some(188_400_000), Some("important=yes"), false, false),
            make_snap("root", 143, "single", None, None, "2026-11-21 08:00:01", "root", Some("timeline"), "timeline", Some(24_100_000), None, false, false),
            make_snap("root", 142, "single", None, None, "2026-11-20 22:15:33", "kaos", None, "Pre-Wayland GPU driver tweak", Some(312_000_000), Some("important=yes,ticket=SYS-42"), false, false),
            make_snap("root", 141, "post", Some(140), None, "2026-11-20 18:10:44", "root", Some("number"), "pacman -S mesa vulkan-radeon", Some(62_500_000), None, false, false),
            make_snap("root", 140, "pre", None, Some(141), "2026-11-20 18:08:11", "root", Some("number"), "pacman -S mesa vulkan-radeon", Some(215_700_000), None, false, false),
            make_snap("root", 139, "single", None, None, "2026-11-20 12:00:01", "root", Some("timeline"), "timeline", Some(18_900_000), None, false, false),
            make_snap("root", 136, "single", None, None, "2026-11-19 19:30:00", "kaos", None, "Baseline workstation setup", Some(642_000_000), Some("important=yes"), false, false),
            make_snap("root", 132, "single", None, None, "2026-11-18 09:00:01", "root", Some("timeline"), "timeline", Some(134_200_000), None, false, false),
            make_snap("home", 0, "single", None, None, "", "root", None, "current", None, None, false, false),
            make_snap("home", 88, "single", None, None, "2026-11-21 10:00:00", "kaos", Some("timeline"), "timeline", Some(14_500_000), None, true, true),
            make_snap("home", 87, "single", None, None, "2026-11-21 09:00:00", "kaos", Some("timeline"), "timeline", Some(31_800_000), None, false, false),
            make_snap("home", 86, "single", None, None, "2026-11-20 21:45:12", "kaos", None, "Before dotfiles Hyprland refactor", Some(428_000_000), Some("important=yes"), false, false),
            make_snap("home", 85, "single", None, None, "2026-11-19 20:00:00", "kaos", Some("timeline"), "timeline", Some(96_400_000), None, false, false),
            make_snap("var-log", 0, "single", None, None, "", "root", None, "current", None, None, false, false),
            make_snap("var-log", 12, "single", None, None, "2026-11-21 06:00:00", "root", Some("timeline"), "timeline", Some(8_420_000), None, true, true),
            make_snap("var-log", 11, "single", None, None, "2026-11-20 06:00:00", "root", Some("timeline"), "timeline", Some(19_600_000), None, false, false),
        ];

        let mut settings = HashMap::new();
        settings.insert("root".to_string(), default_settings("/"));
        settings.insert("home".to_string(), default_settings("/home"));
        settings.insert("var-log".to_string(), default_settings("/var/log"));

        Self {
            state: Mutex::new(MockState {
                configs,
                snapshots,
                settings,
            }),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn make_snap(
    config: &str,
    number: u32,
    snapshot_type: &str,
    pre_number: Option<u32>,
    post_number: Option<u32>,
    date: &str,
    user: &str,
    cleanup: Option<&str>,
    description: &str,
    used_space: Option<u64>,
    userdata_csv: Option<&str>,
    default: bool,
    active: bool,
) -> Snapshot {
    let userdata = userdata_csv.map(|csv| {
        csv.split(',')
            .filter_map(|pair| pair.split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect::<BTreeMap<String, String>>()
    });
    let subvolume = if number == 0 {
        match config {
            "root" => "/".to_string(),
            "home" => "/home".to_string(),
            _ => format!("/{}", config),
        }
    } else if config == "root" {
        format!("/.snapshots/{}/snapshot", number)
    } else {
        format!("/{}/.snapshots/{}/snapshot", config, number)
    };

    Snapshot {
        config: config.to_string(),
        subvolume,
        number,
        snapshot_type: snapshot_type.to_string(),
        pre_number,
        post_number,
        date: date.to_string(),
        user: user.to_string(),
        cleanup: cleanup.map(str::to_string),
        description: description.to_string(),
        userdata,
        used_space,
        default,
        active,
    }
}

fn default_settings(subvolume: &str) -> Vec<ConfigSetting> {
    vec![
        ("ALLOW_GROUPS", ""),
        ("ALLOW_USERS", "kaos"),
        ("BACKGROUND_COMPARISON", "yes"),
        ("EMPTY_PRE_POST_CLEANUP", "yes"),
        ("EMPTY_PRE_POST_MIN_AGE", "1800"),
        ("FREE_LIMIT", "0.2"),
        ("FSTYPE", "btrfs"),
        ("NUMBER_CLEANUP", "yes"),
        ("NUMBER_LIMIT", "20"),
        ("NUMBER_LIMIT_IMPORTANT", "10"),
        ("NUMBER_MIN_AGE", "1800"),
        ("QGROUP", ""),
        ("SPACE_LIMIT", "0.5"),
        ("SUBVOLUME", subvolume),
        ("SYNC_ACL", "yes"),
        ("TIMELINE_CLEANUP", "yes"),
        ("TIMELINE_CREATE", "yes"),
        ("TIMELINE_LIMIT_DAILY", "7"),
        ("TIMELINE_LIMIT_HOURLY", "10"),
        ("TIMELINE_LIMIT_MONTHLY", "6"),
        ("TIMELINE_LIMIT_WEEKLY", "4"),
        ("TIMELINE_LIMIT_YEARLY", "2"),
        ("TIMELINE_MIN_AGE", "1800"),
    ]
    .into_iter()
    .map(|(k, v)| ConfigSetting {
        key: k.to_string(),
        value: v.to_string(),
    })
    .collect()
}

fn mock_changes_for(config: &str, from: u32, to: u32) -> Vec<FileChange> {
    let f = |content, p, u, g, x, a| FileStatusFlags {
        content,
        permissions: p,
        owner: u,
        group: g,
        xattr: x,
        acl: a,
    };

    match config {
        "home" => vec![
            FileChange {
                path: "/home/kaos/.config/hypr/hyprland.conf".to_string(),
                flags: f(ContentChange::Modified, false, false, false, false, false),
            },
            FileChange {
                path: "/home/kaos/.config/waybar/style.css".to_string(),
                flags: f(ContentChange::Modified, false, false, false, false, false),
            },
            FileChange {
                path: "/home/kaos/.config/snapperman/theme.json".to_string(),
                flags: f(ContentChange::Created, false, false, false, false, false),
            },
            FileChange {
                path: "/home/kaos/.local/share/legacy.bak".to_string(),
                flags: f(ContentChange::Deleted, false, false, false, false, false),
            },
        ],
        "var-log" => vec![
            FileChange {
                path: format!("/var/log/snapper-{}-{}.log", from, to),
                flags: f(ContentChange::Created, false, false, false, false, false),
            },
            FileChange {
                path: "/var/log/pacman.log".to_string(),
                flags: f(ContentChange::Modified, false, false, false, false, false),
            },
        ],
        _ => vec![
            FileChange {
                path: "/etc/fstab".to_string(),
                flags: f(ContentChange::Modified, false, false, false, false, false),
            },
            FileChange {
                path: "/etc/pacman.conf".to_string(),
                flags: f(ContentChange::Modified, true, false, false, false, false),
            },
            FileChange {
                path: "/etc/systemd/system/snapper-timeline.timer".to_string(),
                flags: f(ContentChange::Created, false, false, false, false, false),
            },
            FileChange {
                path: "/etc/systemd/system/old-legacy.service".to_string(),
                flags: f(ContentChange::Deleted, false, false, false, false, false),
            },
            FileChange {
                path: "/usr/lib/os-release".to_string(),
                flags: f(ContentChange::Modified, false, false, false, false, false),
            },
            FileChange {
                path: "/usr/bin/snapperman-helper".to_string(),
                flags: f(ContentChange::Unchanged, true, false, false, false, true),
            },
            FileChange {
                path: "/var/cache/pkg/linux-6.12.4.tar.zst".to_string(),
                flags: f(ContentChange::Created, false, false, false, false, false),
            },
        ],
    }
}

fn mock_diff_for_file(config: &str, from: u32, to: u32, path: &str) -> String {
    let snap_root_a = if config == "root" {
        format!("/.snapshots/{}/snapshot", from)
    } else {
        format!("/{}/.snapshots/{}/snapshot", config, from)
    };
    let snap_root_b = if to == 0 {
        String::new()
    } else if config == "root" {
        format!("/.snapshots/{}/snapshot", to)
    } else {
        format!("/{}/.snapshots/{}/snapshot", config, to)
    };

    if path.ends_with(".tar.zst") || path.ends_with(".bin") {
        return format!(
            "--- {a}{p}\n+++ {b}{p}\nBinary files {a}{p} and {b}{p} differ\n",
            a = snap_root_a,
            b = snap_root_b,
            p = path
        );
    }

    if path.ends_with("hyprland.conf") {
        return format!(
            "--- {snap_root_a}{path}\n+++ {snap_root_b}{path}\n@@ -12,7 +12,8 @@\n monitor=,preferred,auto,1\n \n general {{\n-    col.active_border = rgba(6272a4aa)\n+    col.active_border = rgba(bd93f9ee) rgba(8be9fdee) 45deg\n+    border_size = 2\n     layout = dwindle\n }}\n",
        );
    }

    if path.ends_with("pacman.conf") {
        return format!(
            "--- {snap_root_a}{path}\n+++ {snap_root_b}{path}\n@@ -31,6 +31,8 @@\n # Misc options\n Color\n+ILoveCandy\n+ParallelDownloads = 8\n CheckSpace\n VerbosePkgLists\n",
        );
    }

    if path.ends_with("snapper-timeline.timer") {
        return format!(
            "--- /dev/null\n+++ {snap_root_b}{path}\n@@ -0,0 +1,8 @@\n+[Unit]\n+Description=Hourly Timeline of Snapper Snapshots\n+\n+[Timer]\n+OnCalendar=hourly\n+\n+[Install]\n+WantedBy=timers.target\n",
        );
    }

    if path.ends_with("old-legacy.service") {
        return format!(
            "--- {snap_root_a}{path}\n+++ /dev/null\n@@ -1,6 +0,0 @@\n-[Unit]\n-Description=Deprecated Legacy Helper\n-\n-[Service]\n-ExecStart=/usr/bin/legacy\n",
        );
    }

    format!(
        "--- {snap_root_a}{path}\n+++ {snap_root_b}{path}\n@@ -1,6 +1,7 @@\n # Snapshot comparison {from}..{to} ({config})\n-COMPRESSION=lzo\n+COMPRESSION=zstd:3\n+DISCARD=async\n UUID=9f8a1b2c-3d4e-5f60-a1b2-c3d4e5f60708 / btrfs subvol=@,rw,noatime 0 0\n-UUID=11111111-2222-3333-4444-555555555555 /.snapshots btrfs subvol=@snapshots 0 0\n+UUID=11111111-2222-3333-4444-555555555555 /.snapshots btrfs subvol=@snapshots,compress=zstd:3 0 0\n",
    )
}

impl SnapperBackend for MockSnapperBackend {
    fn backend_label(&self) -> String {
        "Mock (Demo)".to_string()
    }

    fn is_mock(&self) -> bool {
        true
    }

    fn list_configs(&self) -> Result<Vec<ConfigInfo>> {
        let state = self.state.lock().unwrap();
        Ok(state.configs.clone())
    }

    fn list_snapshots(
        &self,
        config: Option<&str>,
        include_used_space: bool,
    ) -> Result<Vec<Snapshot>> {
        let state = self.state.lock().unwrap();
        let snaps = state
            .snapshots
            .iter()
            .filter(|s| config.is_none_or(|c| s.config == c))
            .map(|s| {
                let mut cloned = s.clone();
                if !include_used_space {
                    cloned.used_space = None;
                }
                cloned
            })
            .collect();
        Ok(snaps)
    }

    fn create_snapshot(&self, req: &CreateSnapshotRequest) -> Result<Option<u32>> {
        let mut state = self.state.lock().unwrap();
        if !state.configs.iter().any(|c| c.name == req.config) {
            anyhow::bail!("Unknown config '{}'", req.config);
        }
        let next_num = state
            .snapshots
            .iter()
            .filter(|s| s.config == req.config)
            .map(|s| s.number)
            .max()
            .unwrap_or(0)
            + 1;

        let new_snap = Snapshot {
            config: req.config.clone(),
            subvolume: if req.config == "root" {
                format!("/.snapshots/{}/snapshot", next_num)
            } else {
                format!("/{}/.snapshots/{}/snapshot", req.config, next_num)
            },
            number: next_num,
            snapshot_type: req.snapshot_type.as_cli_str().to_string(),
            pre_number: match req.snapshot_type {
                crate::model::SnapshotCreateType::Post { pre_number } => Some(pre_number),
                _ => None,
            },
            post_number: None,
            date: "2026-11-21 11:20:00".to_string(),
            user: "kaos".to_string(),
            cleanup: req.cleanup.as_ref().map(|c| c.as_cli_str().to_string()),
            description: req.description.clone(),
            userdata: if req.userdata.is_empty() {
                None
            } else {
                Some(req.userdata.clone())
            },
            used_space: Some(16_384_000),
            default: false,
            active: false,
        };
        state.snapshots.push(new_snap);
        Ok(Some(next_num))
    }

    fn modify_snapshot(&self, req: &ModifySnapshotRequest) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let snap = state
            .snapshots
            .iter_mut()
            .find(|s| s.key() == req.key)
            .ok_or_else(|| anyhow::anyhow!("Snapshot {} not found", req.key))?;
        if let Some(desc) = &req.description {
            snap.description = desc.clone();
        }
        if let Some(cleanup_opt) = &req.cleanup {
            snap.cleanup = cleanup_opt.as_ref().map(|c| c.as_cli_str().to_string());
        }
        if let Some(ud) = &req.userdata {
            snap.userdata = if ud.is_empty() { None } else { Some(ud.clone()) };
        }
        Ok(())
    }

    fn delete_snapshots(&self, targets: &[SnapshotKey]) -> BatchDeleteReport {
        let mut state = self.state.lock().unwrap();
        let mut report = BatchDeleteReport::default();
        for key in targets {
            if let Some(pos) = state.snapshots.iter().position(|s| s.key() == *key) {
                if state.snapshots[pos].is_protected() {
                    report.failed.push((
                        key.clone(),
                        format!(
                            "Snapshot {} is protected (current, active, or default)",
                            key
                        ),
                    ));
                } else {
                    state.snapshots.remove(pos);
                    report.deleted.push(key.clone());
                }
            } else {
                report
                    .failed
                    .push((key.clone(), format!("Snapshot {} not found", key)));
            }
        }
        report
    }

    fn rollback_snapshot(&self, key: &SnapshotKey) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let mut found = false;
        for snap in &mut state.snapshots {
            if snap.config == key.config {
                if snap.number == key.number {
                    snap.default = true;
                    found = true;
                } else {
                    snap.default = false;
                }
            }
        }
        if !found {
            anyhow::bail!("Snapshot {} not found", key);
        }
        Ok(())
    }

    fn snapshot_status(&self, config: &str, from: u32, to: u32) -> Result<Vec<FileChange>> {
        let state = self.state.lock().unwrap();
        if !state.configs.iter().any(|c| c.name == config) {
            anyhow::bail!("Unknown config '{}'", config);
        }
        Ok(mock_changes_for(config, from, to))
    }

    fn snapshot_diff(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
        max_bytes: usize,
    ) -> Result<DiffOutput> {
        let changes = mock_changes_for(config, from, to);
        let selected: Vec<String> = if files.is_empty() {
            changes.into_iter().map(|c| c.path).collect()
        } else {
            files.to_vec()
        };

        let mut combined = String::new();
        for path in selected {
            combined.push_str(&mock_diff_for_file(config, from, to, &path));
        }

        let total_bytes = combined.len();
        if total_bytes > max_bytes {
            let mut end = max_bytes;
            while !combined.is_char_boundary(end) {
                end = end.saturating_sub(1);
            }
            combined.truncate(end);
            Ok(DiffOutput {
                patch: combined,
                truncated: true,
                total_bytes,
            })
        } else {
            Ok(DiffOutput {
                patch: combined,
                truncated: false,
                total_bytes,
            })
        }
    }

    fn undo_changes(
        &self,
        _config: &str,
        _from: u32,
        _to: u32,
        files: &[String],
    ) -> Result<String> {
        let count = files.len().max(1);
        Ok(format!("create:0 modify:{} delete:0", count))
    }

    fn get_config(&self, config: &str) -> Result<Vec<ConfigSetting>> {
        let state = self.state.lock().unwrap();
        state
            .settings
            .get(config)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Unknown config '{}'", config))
    }

    fn set_config(&self, config: &str, settings: &[ConfigSetting]) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let entry = state
            .settings
            .get_mut(config)
            .ok_or_else(|| anyhow::anyhow!("Unknown config '{}'", config))?;
        for update in settings {
            if update.is_readonly() {
                continue;
            }
            if let Some(existing) = entry.iter_mut().find(|s| s.key == update.key) {
                existing.value = update.value.clone();
            } else {
                entry.push(update.clone());
            }
        }
        Ok(())
    }

    fn run_cleanup(&self, config: &str, algorithm: CleanupAlgorithm) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let alg_str = algorithm.as_cli_str().to_string();
        state.snapshots.retain(|s| {
            if s.config != config || s.is_protected() || s.is_important() {
                return true;
            }
            match algorithm {
                CleanupAlgorithm::All => s.cleanup.is_none(),
                _ => s.cleanup.as_deref() != Some(alg_str.as_str()),
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SnapshotCreateType;

    #[test]
    fn test_mock_crud_and_guards() {
        let mock = MockSnapperBackend::seeded();
        let configs = mock.list_configs().unwrap();
        assert_eq!(configs.len(), 3);

        // Two-phase list: fast without used_space vs with used_space
        let fast = mock.list_snapshots(Some("root"), false).unwrap();
        assert!(fast.iter().all(|s| s.used_space.is_none()));
        let full = mock.list_snapshots(Some("root"), true).unwrap();
        assert!(full.iter().any(|s| s.used_space.is_some()));

        // Create
        let created = mock
            .create_snapshot(&CreateSnapshotRequest {
                config: "home".to_string(),
                description: "test snapshot".to_string(),
                snapshot_type: SnapshotCreateType::Single,
                cleanup: Some(CleanupAlgorithm::Number),
                userdata: BTreeMap::from([("important".to_string(), "yes".to_string())]),
            })
            .unwrap()
            .unwrap();
        assert_eq!(created, 89);

        // Protected delete fails while normal delete succeeds
        let report = mock.delete_snapshots(&[
            SnapshotKey::new("root", 0),
            SnapshotKey::new("root", 145), // active + default
            SnapshotKey::new("home", 89),
        ]);
        assert_eq!(report.success_count(), 1);
        assert_eq!(report.fail_count(), 2);
    }

    #[test]
    fn test_mock_diff_truncation_and_undochange() {
        let mock = MockSnapperBackend::seeded();
        let changes = mock.snapshot_status("root", 144, 145).unwrap();
        assert!(!changes.is_empty());

        let diff = mock
            .snapshot_diff("root", 144, 145, &["/etc/fstab".to_string()], 32)
            .unwrap();
        assert!(diff.truncated);
        assert!(diff.patch.len() <= 32);

        let undo = mock
            .undo_changes("root", 144, 145, &["/etc/fstab".to_string()])
            .unwrap();
        assert!(undo.contains("modify:1"));
    }
}
