use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::{Colorize as _, Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use gpui_kit::{App, Hsla, Image, ImageFormat, hsla, px};
use serde::{Deserialize, Serialize};

pub const AETHERPUNK_SUITE_JSON: &str = include_str!("../assets/themes/aetherpunk-suite.json");
pub const CYBERPUNK_DRACULA_JSON: &str = include_str!("../assets/themes/cyberpunk-dracula.json");

const BG_AETHERPUNK_CITADEL: &[u8] = include_bytes!("../assets/backgrounds/aetherpunk-citadel.webp");
const BG_ARCANEPUNK_ENCLAVE: &[u8] = include_bytes!("../assets/backgrounds/arcanepunk-enclave.webp");
const BG_AETHER_PARCHMENT: &[u8] = include_bytes!("../assets/backgrounds/aether-parchment.webp");
const BG_CYBERPUNK_DRACULA: &[u8] = include_bytes!("../assets/backgrounds/cyberpunk-dracula.webp");
const BG_SYSTEM_DARK: &[u8] = include_bytes!("../assets/backgrounds/system-dark.webp");
const BG_SYSTEM_LIGHT: &[u8] = include_bytes!("../assets/backgrounds/system-light.webp");

pub const AETHERPUNK_CITADEL_NAME: &str = "Aetherpunk Citadel";
pub const ARCANEPUNK_ENCLAVE_NAME: &str = "Arcanepunk Enclave";
pub const AETHER_PARCHMENT_NAME: &str = "Aether Parchment";
pub const CYBERPUNK_DRACULA_NAME: &str = "Cyberpunk Dracula";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AppThemePreset {
    #[default]
    AetherpunkCitadel,
    ArcanepunkEnclave,
    AetherParchment,
    CyberpunkDracula,
    SystemDark,
    SystemLight,
}

impl AppThemePreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::AetherpunkCitadel => AETHERPUNK_CITADEL_NAME,
            Self::ArcanepunkEnclave => ARCANEPUNK_ENCLAVE_NAME,
            Self::AetherParchment => AETHER_PARCHMENT_NAME,
            Self::CyberpunkDracula => CYBERPUNK_DRACULA_NAME,
            Self::SystemDark => "System Dark",
            Self::SystemLight => "System Light",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::AetherpunkCitadel => "Citadel",
            Self::ArcanepunkEnclave => "Enclave",
            Self::AetherParchment => "Parchment",
            Self::CyberpunkDracula => "Cyberpunk",
            Self::SystemDark => "Dark",
            Self::SystemLight => "Light",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::AetherpunkCitadel => "Astral lapis, burnished brass & luminous aether",
            Self::ArcanepunkEnclave => "Obsidian violet, arcane amethyst & alchemy emerald",
            Self::AetherParchment => "Archival ivory parchment, antiqued brass & arcane teal",
            Self::CyberpunkDracula => "Midnight indigo, neon violet & cyber cyan",
            Self::SystemDark => "Default dark workspace palette",
            Self::SystemLight => "Default light workspace palette",
        }
    }

    pub fn icon(self) -> LucideIconName {
        match self {
            Self::AetherpunkCitadel => LucideIconName::Compass,
            Self::ArcanepunkEnclave => LucideIconName::Sparkles,
            Self::AetherParchment => LucideIconName::Sun,
            Self::CyberpunkDracula => LucideIconName::Palette,
            Self::SystemDark => LucideIconName::Moon,
            Self::SystemLight => LucideIconName::Sun,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::AetherpunkCitadel => Self::ArcanepunkEnclave,
            Self::ArcanepunkEnclave => Self::AetherParchment,
            Self::AetherParchment => Self::CyberpunkDracula,
            Self::CyberpunkDracula => Self::SystemDark,
            Self::SystemDark => Self::SystemLight,
            Self::SystemLight => Self::AetherpunkCitadel,
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::AetherpunkCitadel,
            Self::ArcanepunkEnclave,
            Self::AetherParchment,
            Self::CyberpunkDracula,
            Self::SystemDark,
            Self::SystemLight,
        ]
    }
}

/// Returns the cached decoded [`Image`] handle for the given theme preset's
/// abstract solid-object / landscape background artwork.
pub fn theme_background_image(preset: AppThemePreset) -> Arc<Image> {
    static CITADEL: OnceLock<Arc<Image>> = OnceLock::new();
    static ENCLAVE: OnceLock<Arc<Image>> = OnceLock::new();
    static PARCHMENT: OnceLock<Arc<Image>> = OnceLock::new();
    static DRACULA: OnceLock<Arc<Image>> = OnceLock::new();
    static SYS_DARK: OnceLock<Arc<Image>> = OnceLock::new();
    static SYS_LIGHT: OnceLock<Arc<Image>> = OnceLock::new();

    let (slot, bytes) = match preset {
        AppThemePreset::AetherpunkCitadel => (&CITADEL, BG_AETHERPUNK_CITADEL),
        AppThemePreset::ArcanepunkEnclave => (&ENCLAVE, BG_ARCANEPUNK_ENCLAVE),
        AppThemePreset::AetherParchment => (&PARCHMENT, BG_AETHER_PARCHMENT),
        AppThemePreset::CyberpunkDracula => (&DRACULA, BG_CYBERPUNK_DRACULA),
        AppThemePreset::SystemDark => (&SYS_DARK, BG_SYSTEM_DARK),
        AppThemePreset::SystemLight => (&SYS_LIGHT, BG_SYSTEM_LIGHT),
    };

    slot.get_or_init(|| Arc::new(Image::from_bytes(ImageFormat::Webp, bytes.to_vec())))
        .clone()
}

/// Theme-adaptive Aetherpunk accent colors derived from the active [`Theme`] tokens,
/// avoiding hardcoded hex values inside view code across all 6 presets.
#[derive(Debug, Clone, Copy)]
pub struct ThemeAccents {
    pub brass_rail: Hsla,
    pub brass_hairline: Hsla,
    pub conduit_glow: Hsla,
    pub plaque_bg: Hsla,
    pub header_tint: Hsla,
    pub sunken_bg: Hsla,
    pub badge_tint: Hsla,
    pub orb_halo: Hsla,
    pub subtle_border: Hsla,
    pub shadow_primary: Hsla,
    pub shadow_ambient: Hsla,
    pub backdrop_scrim: Hsla,
    pub chrome_bg: Hsla,
    pub surface_panel: Hsla,
    pub surface_sidebar: Hsla,
    pub surface_card: Hsla,
    pub surface_diff: Hsla,
}

impl ThemeAccents {
    pub fn from_theme(theme: &Theme) -> Self {
        let is_dark = theme.mode.is_dark();
        Self {
            brass_rail: theme.primary.opacity(if is_dark { 0.88 } else { 0.82 }),
            brass_hairline: theme
                .title_bar_border
                .mix_oklab(theme.primary.opacity(0.42), 0.55),
            conduit_glow: theme.info.opacity(if is_dark { 0.85 } else { 0.9 }),
            plaque_bg: theme.group_box,
            header_tint: theme
                .group_box
                .mix_oklab(theme.muted, if is_dark { 0.35 } else { 0.45 })
                .opacity(if is_dark { 0.26 } else { 0.24 }),
            sunken_bg: theme
                .background
                .mix_oklab(theme.group_box, if is_dark { 0.42 } else { 0.55 })
                .opacity(if is_dark { 0.24 } else { 0.22 }),
            badge_tint: theme.primary.opacity(if is_dark { 0.16 } else { 0.12 }),
            orb_halo: theme.info.opacity(if is_dark { 0.22 } else { 0.16 }),
            subtle_border: theme.border,
            shadow_primary: if is_dark {
                hsla(0.62, 0.45, 0.02, 0.42)
            } else {
                hsla(0.10, 0.25, 0.20, 0.11)
            },
            shadow_ambient: if is_dark {
                hsla(0.62, 0.45, 0.02, 0.24)
            } else {
                hsla(0.10, 0.25, 0.20, 0.06)
            },
            backdrop_scrim: theme.background.opacity(if is_dark { 0.08 } else { 0.03 }),
            chrome_bg: theme.background.opacity(if is_dark { 0.36 } else { 0.30 }),
            surface_panel: theme.group_box.opacity(if is_dark { 0.46 } else { 0.40 }),
            surface_sidebar: theme.sidebar.opacity(if is_dark { 0.40 } else { 0.34 }),
            surface_card: theme.group_box.opacity(if is_dark { 0.44 } else { 0.38 }),
            surface_diff: theme.group_box.opacity(if is_dark { 0.62 } else { 0.58 }),
        }
    }
}

pub fn init_themes(cx: &mut App, initial: AppThemePreset) {
    let registry = ThemeRegistry::global_mut(cx);
    let _ = registry.load_themes_from_str(AETHERPUNK_SUITE_JSON);
    let _ = registry.load_themes_from_str(CYBERPUNK_DRACULA_JSON);
    apply_theme_preset(initial, cx);
}

pub fn apply_theme_preset(preset: AppThemePreset, cx: &mut App) {
    let registry = ThemeRegistry::global(cx);
    let (mode, config): (ThemeMode, Rc<ThemeConfig>) = match preset {
        AppThemePreset::AetherpunkCitadel => (
            ThemeMode::Dark,
            registry
                .themes()
                .get(AETHERPUNK_CITADEL_NAME)
                .cloned()
                .unwrap_or_else(|| registry.default_dark_theme().clone()),
        ),
        AppThemePreset::ArcanepunkEnclave => (
            ThemeMode::Dark,
            registry
                .themes()
                .get(ARCANEPUNK_ENCLAVE_NAME)
                .cloned()
                .unwrap_or_else(|| registry.default_dark_theme().clone()),
        ),
        AppThemePreset::AetherParchment => (
            ThemeMode::Light,
            registry
                .themes()
                .get(AETHER_PARCHMENT_NAME)
                .cloned()
                .unwrap_or_else(|| registry.default_light_theme().clone()),
        ),
        AppThemePreset::CyberpunkDracula => (
            ThemeMode::Dark,
            registry
                .themes()
                .get(CYBERPUNK_DRACULA_NAME)
                .cloned()
                .unwrap_or_else(|| registry.default_dark_theme().clone()),
        ),
        AppThemePreset::SystemDark => (ThemeMode::Dark, registry.default_dark_theme().clone()),
        AppThemePreset::SystemLight => (ThemeMode::Light, registry.default_light_theme().clone()),
    };

    Theme::update(cx, |theme| {
        if mode.is_dark() {
            theme.dark_theme = config;
        } else {
            theme.light_theme = config;
        }
    });
    Theme::change(mode, None, cx);

    if matches!(preset, AppThemePreset::SystemDark | AppThemePreset::SystemLight) {
        let is_dark = mode.is_dark();
        let none = gpui_kit::transparent_black();
        Theme::update(cx, |theme| {
            theme.radius = px(10.);
            theme.radius_lg = px(16.);
            theme.colors.table = theme.colors.table.opacity(if is_dark { 0.45 } else { 0.40 });
            theme.colors.table_head = theme.colors.table_head.opacity(if is_dark { 0.50 } else { 0.47 });
            theme.colors.table_foot = theme.colors.table_foot.opacity(if is_dark { 0.50 } else { 0.47 });
            theme.colors.table_hover = theme.colors.table_hover.opacity(if is_dark { 0.48 } else { 0.45 });
            theme.colors.table_even = theme.colors.table_even.opacity(0.09);
            theme.colors.border = none;
            theme.colors.sidebar_border = none;
            theme.colors.table_row_border = none;
            theme.colors.title_bar_border = none;
            theme.colors.status_bar_border = none;
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use gpui_kit::component::{Theme, ThemeSet, try_parse_color};

    fn assert_schema_keys_populated(cfg: &ThemeConfig) {
        let name = cfg.name.as_ref();
        assert!(
            cfg.colors.description_list_label.is_some(),
            "{name}: missing description_list.label.background"
        );
        assert!(
            cfg.colors.description_list_label_foreground.is_some(),
            "{name}: missing description_list.label.foreground"
        );
        assert!(cfg.colors.drag_border.is_some(), "{name}: missing drag.border");
        assert!(cfg.colors.link.is_some(), "{name}: missing link");
        assert!(cfg.colors.link_active.is_some(), "{name}: missing link.active");
        assert!(cfg.colors.link_hover.is_some(), "{name}: missing link.hover");
        assert!(
            cfg.colors.progress_bar.is_some(),
            "{name}: missing progress.bar.background"
        );
        assert!(
            cfg.colors.slider_bar.is_some(),
            "{name}: missing slider.background"
        );
        assert!(cfg.highlight.is_some(), "{name}: missing highlight block");
    }

    #[test]
    fn aetherpunk_suite_json_parses_cleanly_and_applies_configs() {
        let parsed: ThemeSet =
            serde_json::from_str(AETHERPUNK_SUITE_JSON).expect("aetherpunk suite should parse");
        assert_eq!(parsed.name, "Aetherpunk Suite");
        assert_eq!(parsed.themes.len(), 3);

        let expected = [
            (AETHERPUNK_CITADEL_NAME, ThemeMode::Dark, true),
            (ARCANEPUNK_ENCLAVE_NAME, ThemeMode::Dark, false),
            (AETHER_PARCHMENT_NAME, ThemeMode::Light, false),
        ];

        for (theme_cfg, (name, mode, is_default)) in parsed.themes.iter().zip(expected) {
            assert_eq!(theme_cfg.name, name);
            assert_eq!(theme_cfg.mode, mode);
            assert_eq!(theme_cfg.is_default, is_default);
            assert_eq!(theme_cfg.radius, Some(10));
            assert_eq!(theme_cfg.radius_lg, Some(16));
            assert_schema_keys_populated(theme_cfg);

            let mut applied = Theme::default();
            let rc = Rc::new(theme_cfg.clone());
            applied.apply_config(&rc);
            assert_eq!(applied.mode, mode);
            assert_eq!(
                applied.colors.link,
                try_parse_color(theme_cfg.colors.link.as_ref().unwrap()).unwrap(),
                "{name}: link color was not applied"
            );
            assert_ne!(
                applied.tokens.primary.background,
                gpui_kit::Background::from(applied.colors.primary),
                "{name}: primary gradient background should not collapse to a solid color"
            );
            assert_ne!(
                applied.tokens.title_bar.background,
                gpui_kit::Background::from(applied.colors.title_bar),
                "{name}: title_bar gradient background should not collapse to a solid color"
            );
        }
    }

    #[test]
    fn cyberpunk_dracula_json_parses_cleanly_with_repaired_keys() {
        let parsed: ThemeSet =
            serde_json::from_str(CYBERPUNK_DRACULA_JSON).expect("theme JSON should parse");
        assert_eq!(parsed.name, "Cyberpunk Dracula");
        assert_eq!(parsed.themes.len(), 1);
        let theme = &parsed.themes[0];
        assert_eq!(theme.name, CYBERPUNK_DRACULA_NAME);
        assert_eq!(theme.mode, ThemeMode::Dark);
        assert!(!theme.is_default);
        assert_eq!(theme.radius, Some(10));
        assert_eq!(theme.radius_lg, Some(16));
        assert_schema_keys_populated(theme);
    }

    #[test]
    fn theme_preset_cycle_covers_all_variants() {
        let all = AppThemePreset::all();
        assert_eq!(all.len(), 6);
        let mut cursor = AppThemePreset::default();
        assert_eq!(cursor, AppThemePreset::AetherpunkCitadel);
        let mut visited = Vec::with_capacity(all.len());
        for _ in 0..all.len() {
            visited.push(cursor);
            cursor = cursor.next();
        }
        assert_eq!(visited.as_slice(), all);
        assert_eq!(cursor, AppThemePreset::AetherpunkCitadel);
    }

    #[test]
    fn theme_background_images_are_valid_webp_and_distinct() {
        let mut ids = BTreeSet::new();
        for preset in AppThemePreset::all().iter().copied() {
            let image = theme_background_image(preset);
            assert_eq!(image.format, ImageFormat::Webp);
            assert!(
                image.bytes.len() > 1024,
                "{:?}: background artwork unexpectedly tiny",
                preset
            );
            assert_eq!(&image.bytes[..4], b"RIFF");
            assert_eq!(&image.bytes[8..12], b"WEBP");
            assert!(
                ids.insert(image.id()),
                "{:?}: duplicate background image hash",
                preset
            );
        }
        assert_eq!(ids.len(), 6);
    }
}
