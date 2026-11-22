use crate::model::{
    ConfigInfo, ConfigSetting, ContentChange, FileChange, FileStatusFlags, FileTreeNode, Snapshot,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

/// Parse `snapper --jsonout list` payload into flattened `Snapshot` entries.
pub fn parse_snapshots_json(raw: &str) -> Result<Vec<Snapshot>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let payload: HashMap<String, Vec<Snapshot>> =
        serde_json::from_str(trimmed).context("Failed to parse snapper list JSON output")?;

    let mut snapshots = Vec::new();
    for (config_name, mut entries) in payload {
        for entry in &mut entries {
            entry.config = config_name.clone();
        }
        snapshots.append(&mut entries);
    }
    Ok(snapshots)
}

#[derive(Deserialize)]
struct JsonConfigEntry {
    config: String,
    subvolume: String,
}

#[derive(Deserialize)]
struct JsonConfigsPayload {
    configs: Vec<JsonConfigEntry>,
}

/// Parse `snapper --jsonout list-configs` payload (supports JSON object wrapper, JSON array, or text table fallback).
pub fn parse_configs_json(raw: &str) -> Result<Vec<ConfigInfo>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if let Ok(payload) = serde_json::from_str::<JsonConfigsPayload>(trimmed) {
        return Ok(payload
            .configs
            .into_iter()
            .map(|c| ConfigInfo {
                name: c.config,
                subvolume: c.subvolume,
            })
            .collect());
    }

    if let Ok(entries) = serde_json::from_str::<Vec<JsonConfigEntry>>(trimmed) {
        return Ok(entries
            .into_iter()
            .map(|c| ConfigInfo {
                name: c.config,
                subvolume: c.subvolume,
            })
            .collect());
    }

    // Fallback for plain text `Config | Subvolume` table
    let mut parsed = Vec::new();
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Config") || line.contains("---+---") {
            continue;
        }
        if let Some((cfg, subvol)) = line.split_once('|') {
            let name = cfg.trim();
            let subvolume = subvol.trim();
            if !name.is_empty() && !subvolume.is_empty() {
                parsed.push(ConfigInfo {
                    name: name.to_string(),
                    subvolume: subvolume.to_string(),
                });
            }
        }
    }
    if !parsed.is_empty() {
        return Ok(parsed);
    }

    anyhow::bail!("Failed to parse snapper list-configs output")
}

/// Parse `snapper --jsonout get-config` key-value map.
pub fn parse_config_settings_json(raw: &str) -> Result<Vec<ConfigSetting>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let map: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(trimmed).context("Failed to parse snapper get-config JSON output")?;

    Ok(map
        .into_iter()
        .map(|(key, val)| {
            let value = match val {
                serde_json::Value::String(s) => s,
                serde_json::Value::Bool(b) => {
                    if b {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }
                }
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            };
            ConfigSetting { key, value }
        })
        .collect())
}

/// Parse a single line from `snapper status`, e.g. `c..... /etc/fstab`.
pub fn parse_status_line(line: &str) -> Option<FileChange> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.trim().is_empty() {
        return None;
    }

    let (flags_str, rest) = trimmed.split_once(char::is_whitespace)?;
    let path = rest.trim_start();
    if path.is_empty() {
        return None;
    }

    let bytes = flags_str.as_bytes();
    if bytes.len() < 5 || bytes.len() > 6 {
        return None;
    }

    let valid_chars = bytes
        .iter()
        .all(|b| matches!(b, b'+' | b'-' | b'c' | b'p' | b'u' | b'g' | b'x' | b'a' | b'.'));
    if !valid_chars {
        return None;
    }

    let content = match bytes[0] {
        b'+' => ContentChange::Created,
        b'-' => ContentChange::Deleted,
        b'c' => ContentChange::Modified,
        b'.' => ContentChange::Unchanged,
        _ => return None,
    };

    let flags = FileStatusFlags {
        content,
        permissions: bytes.get(1) == Some(&b'p'),
        owner: bytes.get(2) == Some(&b'u'),
        group: bytes.get(3) == Some(&b'g'),
        xattr: bytes.get(4) == Some(&b'x'),
        acl: bytes.get(5) == Some(&b'a'),
    };

    Some(FileChange {
        path: path.to_string(),
        flags,
    })
}

/// Parse multi-line `snapper status` output.
pub fn parse_status_output(raw: &str) -> Vec<FileChange> {
    raw.lines().filter_map(parse_status_line).collect()
}

#[derive(Default)]
struct TrieNode {
    label: String,
    full_path: String,
    change: Option<FileChange>,
    children: BTreeMap<String, TrieNode>,
}

/// Build a sorted, path-collapsed file tree from flat `FileChange` entries.
pub fn build_file_tree(changes: &[FileChange]) -> Vec<FileTreeNode> {
    let mut root = TrieNode::default();

    for change in changes {
        let cleaned = change.path.trim_start_matches('/');
        if cleaned.is_empty() {
            continue;
        }
        let segments: Vec<&str> = cleaned.split('/').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            continue;
        }

        let mut cursor = &mut root;
        let mut acc_path = String::new();

        for (idx, seg) in segments.iter().enumerate() {
            if change.path.starts_with('/') || !acc_path.is_empty() {
                acc_path.push('/');
            }
            acc_path.push_str(seg);

            let is_last = idx + 1 == segments.len();
            cursor = cursor.children.entry((*seg).to_string()).or_insert_with(|| {
                TrieNode {
                    label: (*seg).to_string(),
                    full_path: acc_path.clone(),
                    change: None,
                    children: BTreeMap::new(),
                }
            });
            if is_last {
                cursor.change = Some(change.clone());
            }
        }
    }

    finalize_children(root.children)
}

fn finalize_children(children: BTreeMap<String, TrieNode>) -> Vec<FileTreeNode> {
    let mut nodes: Vec<FileTreeNode> = children
        .into_values()
        .map(|node| {
            let mut finalized = FileTreeNode {
                id: node.full_path.clone(),
                label: node.label,
                full_path: node.full_path,
                change: node.change,
                children: finalize_children(node.children),
            };
            collapse_single_child_folder(&mut finalized);
            finalized
        })
        .collect();

    nodes.sort_by(|a, b| {
        let a_is_folder = !a.children.is_empty() && a.change.is_none();
        let b_is_folder = !b.children.is_empty() && b.change.is_none();
        b_is_folder
            .cmp(&a_is_folder)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
            .then_with(|| a.label.cmp(&b.label))
    });

    nodes
}

fn collapse_single_child_folder(node: &mut FileTreeNode) {
    while node.change.is_none() && node.children.len() == 1 {
        let child_is_folder =
            node.children[0].change.is_none() && !node.children[0].children.is_empty();
        if !child_is_folder {
            break;
        }
        let only_child = node.children.remove(0);
        node.label = format!("{}/{}", node.label, only_child.label);
        node.full_path = only_child.full_path;
        node.id = only_child.id;
        node.children = only_child.children;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_parsing_and_empty_string_optionals() {
        let json_data = r#"
        {
            "root": [
                {
                    "active": true,
                    "cleanup": "number",
                    "date": "2023-10-27 10:00:00",
                    "default": false,
                    "description": "timeline",
                    "number": 100,
                    "post-number": 101,
                    "pre-number": 99,
                    "subvolume": "/.snapshots/100/snapshot",
                    "type": "single",
                    "used-space": 12345,
                    "user": "root",
                    "userdata": {
                        "important": "yes"
                    }
                },
                {
                    "active": false,
                    "cleanup": "",
                    "date": "",
                    "default": false,
                    "description": "current",
                    "number": 0,
                    "post-number": "",
                    "pre-number": "",
                    "subvolume": "/",
                    "type": "single",
                    "used-space": "",
                    "user": "root",
                    "userdata": null
                }
            ]
        }
        "#;

        let snapshots = parse_snapshots_json(json_data).unwrap();
        assert_eq!(snapshots.len(), 2);
        let snap = &snapshots[0];
        assert_eq!(snap.config, "root");
        assert_eq!(snap.number, 100);
        assert_eq!(snap.snapshot_type, "single");
        assert_eq!(snap.used_space, Some(12345));
        assert!(snap.active);
        assert!(snap.is_important());

        let current = &snapshots[1];
        assert_eq!(current.number, 0);
        assert_eq!(current.cleanup, None);
        assert_eq!(current.pre_number, None);
        assert_eq!(current.post_number, None);
        assert_eq!(current.used_space, None);
    }

    #[test]
    fn test_configs_json_and_text_fallback() {
        let cfg_json = r#"{"configs":[{"config":"root","subvolume":"/"},{"config":"home","subvolume":"/home"}]}"#;
        let configs = parse_configs_json(cfg_json).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].name, "root");
        assert_eq!(configs[1].subvolume, "/home");

        let cfg_text = "Config | Subvolume\n-------+----------\nroot   | /\nhome   | /home\n";
        let text_configs = parse_configs_json(cfg_text).unwrap();
        assert_eq!(text_configs, configs);

        let settings_json = r#"{"SUBVOLUME":"/","FSTYPE":"btrfs","NUMBER_CLEANUP":"yes","NUMBER_LIMIT":"10"}"#;
        let settings = parse_config_settings_json(settings_json).unwrap();
        assert_eq!(settings.len(), 4);
        assert!(settings.iter().find(|s| s.key == "SUBVOLUME").unwrap().is_readonly());
        assert!(!settings.iter().find(|s| s.key == "NUMBER_LIMIT").unwrap().is_readonly());
    }

    #[test]
    fn test_status_flag_matrix_and_spaces() {
        let raw = "\
+..... /etc/new.conf
-..... /etc/old.conf
cpugxa /etc/my file with spaces.conf
.p.... /usr/bin/tool
invalid header line
";
        let changes = parse_status_output(raw);
        assert_eq!(changes.len(), 4);
        assert_eq!(changes[0].flags.content, ContentChange::Created);
        assert_eq!(changes[0].path, "/etc/new.conf");
        assert_eq!(changes[1].flags.content, ContentChange::Deleted);
        assert_eq!(changes[2].flags.content, ContentChange::Modified);
        assert!(changes[2].flags.permissions);
        assert!(changes[2].flags.owner);
        assert!(changes[2].flags.group);
        assert!(changes[2].flags.xattr);
        assert!(changes[2].flags.acl);
        assert_eq!(changes[2].path, "/etc/my file with spaces.conf");
        assert_eq!(changes[3].flags.content, ContentChange::Unchanged);
        assert!(changes[3].flags.permissions);
        assert_eq!(changes[3].flags.summary_badge(), "Attr");
    }

    #[test]
    fn test_build_file_tree_collapses_single_child_folders() {
        let changes = parse_status_output(
            "\
c..... /etc/systemd/system/a.service
+..... /etc/systemd/system/b.service
-..... /var/log/old.log
",
        );
        let tree = build_file_tree(&changes);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].label, "etc/systemd/system");
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[1].label, "var/log");
        assert_eq!(tree[1].children[0].label, "old.log");
    }
}
