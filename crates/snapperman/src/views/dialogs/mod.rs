pub mod auth;
pub mod compare;
pub mod confirm;
pub mod create;
pub mod modify;

use std::collections::BTreeMap;

use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::select::SelectItem;
use gpui_kit::component::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{App, Hsla, IntoElement, ParentElement as _, SharedString, Styled as _, div, px};

use crate::views::aether;

/// Shared soft Organic Aetherpunk icon-led header banner + wave separator used across modal dialogs.
pub fn dialog_header_banner(
    icon: LucideIconName,
    accent: Hsla,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cx: &App,
) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2p5()
        .child(
            h_flex()
                .w_full()
                .gap_3()
                .items_center()
                .p_3p5()
                .rounded(px(12.))
                .bg(accent.opacity(0.10))
                .child(aether::icon_medallion(icon, accent, 34.))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().foreground)
                                .child(title.into()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(subtitle.into()),
                        ),
                ),
        )
        .child(aether::filigree_wave(cx))
}

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
