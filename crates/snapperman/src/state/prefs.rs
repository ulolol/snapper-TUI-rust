use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::AppThemePreset;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct AppPrefs {
    pub theme: AppThemePreset,
    pub sidebar_collapsed: bool,
    pub split_diff: bool,
}

impl AppPrefs {
    pub fn prefs_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("snapperman").join("prefs.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::prefs_path() else {
            return Self::default();
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::prefs_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}
