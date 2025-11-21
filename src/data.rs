use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub config: String,
    #[serde(default)]
    pub subvolume: String,
    #[serde(default)]
    pub number: u32,
    #[serde(rename = "type", default)]
    pub snapshot_type: String,
    #[serde(rename = "pre-number")]
    pub pre_number: Option<u32>,
    #[serde(rename = "post-number")]
    pub post_number: Option<u32>,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub user: String,
    pub cleanup: Option<String>,
    #[serde(default)]
    pub description: String,
    pub userdata: Option<HashMap<String, String>>,
    #[serde(rename = "used-space")]
    pub used_space: Option<u64>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub active: bool,
}

pub fn list_snapshots() -> Result<Vec<Snapshot>> {
    let output = Command::new("snapper")
        .args(&[
            "--jsonout",
            "list",
            "--columns",
            "config,subvolume,number,type,pre-number,post-number,date,user,cleanup,description,userdata,used-space,default,active",
        ])
        .output()
        .context("Failed to execute snapper command")?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Snapper failed: {}", error_msg);
    }

    let output_str = String::from_utf8(output.stdout)?;
    let payload: HashMap<String, Vec<Snapshot>> = serde_json::from_str(&output_str)
        .context("Failed to parse snapper JSON output")?;

    let mut snapshots = Vec::new();
    for (config_name, mut entries) in payload {
        for entry in &mut entries {
            entry.config = config_name.clone();
        }
        snapshots.append(&mut entries);
    }

    Ok(snapshots)
}

pub fn delete_snapshot(number: u32) -> Result<()> {
    let status = Command::new("sudo")
        .args(&["snapper", "delete", &number.to_string()])
        .status()
        .context("Failed to execute snapper delete")?;

    if !status.success() {
        anyhow::bail!("Failed to delete snapshot {}", number);
    }
    Ok(())
}

pub fn rollback_snapshot(number: u32) -> Result<()> {
    let status = Command::new("sudo")
        .args(&["snapper", "rollback", &number.to_string()])
        .status()
        .context("Failed to execute snapper rollback")?;

    if !status.success() {
        anyhow::bail!("Failed to rollback to snapshot {}", number);
    }
    Ok(())
}

pub fn get_snapshot_status(snap: &Snapshot) -> Result<String> {
    let start = snap.pre_number.unwrap_or_else(|| snap.number.saturating_sub(1));
    let range = format!("{}..{}", start, snap.number);
    
    let output = Command::new("sudo")
        .args(&["snapper", "status", &range])
        .output()
        .context("Failed to execute snapper status")?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Snapper status failed: {}", error_msg);
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn create_snapshot(description: &str) -> Result<()> {
    let status = Command::new("sudo")
        .args(&["snapper", "create", "--description", description])
        .status()
        .context("Failed to execute snapper create")?;

    if !status.success() {
        anyhow::bail!("Failed to create snapshot");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_parsing() {
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
                }
            ]
        }
        "#;

        let payload: HashMap<String, Vec<Snapshot>> = serde_json::from_str(json_data).unwrap();
        let snapshots = payload.get("root").unwrap();
        assert_eq!(snapshots.len(), 1);
        let snap = &snapshots[0];
        assert_eq!(snap.number, 100);
        assert_eq!(snap.snapshot_type, "single");
        assert_eq!(snap.used_space, Some(12345));
        assert!(snap.active);
        assert_eq!(snap.userdata.as_ref().unwrap().get("important").unwrap(), "yes");
    }
}
