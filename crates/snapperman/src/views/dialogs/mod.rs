pub mod auth;
pub mod compare;
pub mod confirm;
pub mod create;
pub mod modify;

use std::collections::BTreeMap;

use gpui_kit::SharedString;
use gpui_kit::component::select::SelectItem;

#[derive(Clone, Debug)]
pub struct ChoiceItem {
    pub value: String,
    pub label: SharedString,
}

impl ChoiceItem {
    pub fn new(value: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

impl SelectItem for ChoiceItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub fn parse_userdata_pairs(raw: &str) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(map);
    }
    for part in trimmed.split(',') {
        let entry = part.trim();
        if entry.is_empty() {
            continue;
        }
        let Some((k, v)) = entry.split_once('=') else {
            return Err(format!(
                "Invalid userdata entry '{entry}' (expected comma-separated key=value pairs)."
            ));
        };
        let key = k.trim();
        let val = v.trim();
        if key.is_empty() {
            return Err("Userdata key cannot be empty.".to_string());
        }
        map.insert(key.to_string(), val.to_string());
    }
    Ok(map)
}
