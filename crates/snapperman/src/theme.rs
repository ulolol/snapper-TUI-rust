use std::rc::Rc;

use gpui_kit::App;
use gpui_kit::component::{Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use serde::{Deserialize, Serialize};

pub const CYBERPUNK_DRACULA_JSON: &str = include_str!("../assets/themes/cyberpunk-dracula.json");
pub const CYBERPUNK_DRACULA_NAME: &str = "Cyberpunk Dracula";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AppThemePreset {
    #[default]
    CyberpunkDracula,
    SystemDark,
    SystemLight,
}

impl AppThemePreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::CyberpunkDracula => "Cyberpunk Dracula",
            Self::SystemDark => "System Dark",
            Self::SystemLight => "System Light",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::CyberpunkDracula => "Cyberpunk",
            Self::SystemDark => "Dark",
            Self::SystemLight => "Light",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::CyberpunkDracula => Self::SystemDark,
            Self::SystemDark => Self::SystemLight,
            Self::SystemLight => Self::CyberpunkDracula,
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::CyberpunkDracula, Self::SystemDark, Self::SystemLight]
    }
}

pub fn init_themes(cx: &mut App, initial: AppThemePreset) {
    let _ = ThemeRegistry::global_mut(cx).load_themes_from_str(CYBERPUNK_DRACULA_JSON);
    apply_theme_preset(initial, cx);
}

pub fn apply_theme_preset(preset: AppThemePreset, cx: &mut App) {
    let (mode, config): (ThemeMode, Rc<ThemeConfig>) = match preset {
        AppThemePreset::CyberpunkDracula => {
            let cfg = ThemeRegistry::global(cx)
                .themes()
                .get(CYBERPUNK_DRACULA_NAME)
                .cloned()
                .unwrap_or_else(|| {
                    ThemeRegistry::global(cx).default_dark_theme().clone()
                });
            (ThemeMode::Dark, cfg)
        }
        AppThemePreset::SystemDark => (
            ThemeMode::Dark,
            ThemeRegistry::global(cx).default_dark_theme().clone(),
        ),
        AppThemePreset::SystemLight => (
            ThemeMode::Light,
            ThemeRegistry::global(cx).default_light_theme().clone(),
        ),
    };

    Theme::update(cx, |theme| {
        if mode.is_dark() {
            theme.dark_theme = config;
        } else {
            theme.light_theme = config;
        }
    });
    Theme::change(mode, None, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::ThemeSet;

    #[test]
    fn cyberpunk_dracula_json_parses_cleanly() {
        let parsed: ThemeSet =
            serde_json::from_str(CYBERPUNK_DRACULA_JSON).expect("theme JSON should parse");
        assert_eq!(parsed.name, "Cyberpunk Dracula");
        assert_eq!(parsed.themes.len(), 1);
        let theme = &parsed.themes[0];
        assert_eq!(theme.name, CYBERPUNK_DRACULA_NAME);
        assert_eq!(theme.mode, ThemeMode::Dark);
        assert!(theme.highlight.is_some());
    }
}
