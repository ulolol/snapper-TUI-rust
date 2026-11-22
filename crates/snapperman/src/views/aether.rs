use std::rc::Rc;

use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::base::{ResizeHandleRenderer, ResizeHandleState};
use gpui_kit::component::button::ButtonRounded;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, App, Axis, Div, ElementId, Hsla, InteractiveElement as _, IntoElement, Keystroke,
    ObjectFit, ParentElement as _, PathBuilder, Pixels, Point, SharedString, Stateful,
    Styled as _, StyledImage as _, TestSupportExt as _, canvas, div, img, point,
    prelude::FluentBuilder as _, px,
};

use crate::theme::{AppThemePreset, ThemeAccents, theme_background_image};

/// Full-window theme-matched sculptural landscape artwork with an atmospheric scrim
/// tuned to keep surrounding workspace chrome and unified glass pane effortlessly legible.
pub fn theme_backdrop(preset: AppThemePreset, cx: &App) -> impl IntoElement {
    let accents = ThemeAccents::from_theme(cx.theme());
    div()
        .id("theme-backdrop-layer")
        .absolute()
        .inset_0()
        .size_full()
        .overflow_hidden()
        .child(
            img(theme_background_image(preset))
                .size_full()
                .object_fit(ObjectFit::Cover),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .size_full()
                .bg(accents.backdrop_scrim),
        )
}

/// Full pill corner radius for tags, filter chips, tabs, and action buttons.
pub fn pill_radius() -> ButtonRounded {
    ButtonRounded::Size(px(9999.))
}

fn push_circle_subpath(builder: &mut PathBuilder, center: Point<Pixels>, r: Pixels) {
    let left = point(center.x - r, center.y);
    let right = point(center.x + r, center.y);
    let radii = point(r, r);
    builder.move_to(left);
    builder.arc_to(radii, px(0.), false, true, right);
    builder.arc_to(radii, px(0.), false, true, left);
    builder.close();
}

/// Celestial orbital astrolabe/chronometer crest rendered via smooth circular GPUI paths.
/// Uses active theme `primary` (brass/amethyst) and `info` (aether cyan) tokens.
pub fn brand_sigil(size: Pixels, cx: &App) -> impl IntoElement {
    let outer_color = cx.theme().primary;
    let orbit_color = cx.theme().primary.opacity(0.60);
    let halo_color = cx.theme().info.opacity(0.24);
    let core_color = cx.theme().info;

    canvas(
        move |_, _, _| {},
        move |bounds, _, window, _| {
            let center = point(
                bounds.origin.x + bounds.size.width * 0.5,
                bounds.origin.y + bounds.size.height * 0.5,
            );
            let r = (bounds.size.width.min(bounds.size.height)) * 0.43;

            // Outer celestial astrolabe ring
            let mut ring = PathBuilder::stroke(px(1.35));
            push_circle_subpath(&mut ring, center, r);
            if let Ok(path) = ring.build() {
                window.paint_path(path, outer_color);
            }

            // Gentle tilted inner orbital ellipse arc
            let mut orbit = PathBuilder::stroke(px(1.0));
            let ox = r * 0.82;
            let oy = r * 0.42;
            orbit.move_to(point(center.x - ox, center.y + r * 0.14));
            orbit.cubic_bezier_to(
                point(center.x + ox, center.y - r * 0.14),
                point(center.x - ox * 0.35, center.y - oy * 1.15),
                point(center.x + ox * 0.55, center.y - oy * 0.72),
            );
            if let Ok(path) = orbit.build() {
                window.paint_path(path, orbit_color);
            }

            // Soft glowing core halo
            let mut halo = PathBuilder::fill();
            push_circle_subpath(&mut halo, center, r * 0.38);
            if let Ok(path) = halo.build() {
                window.paint_path(path, halo_color);
            }

            // Luminous spherical aether core
            let mut core = PathBuilder::fill();
            push_circle_subpath(&mut core, center, r * 0.21);
            if let Ok(path) = core.build() {
                window.paint_path(path, core_color);
            }
        },
    )
    .size(size)
    .flex_shrink_0()
}

/// Delicate sinuous Art Nouveau S-wave divider with a soft centered pearl orb.
pub fn filigree_wave(cx: &App) -> impl IntoElement {
    let accents = ThemeAccents::from_theme(cx.theme());
    let wave_color = accents.brass_hairline;
    let halo_color = accents.brass_rail.opacity(0.22);
    let pearl_color = accents.brass_rail;

    div().w_full().h(px(12.)).flex_shrink_0().child(
        canvas(
            move |_, _, _| {},
            move |bounds, _, window, _| {
                let y = bounds.origin.y + bounds.size.height * 0.5;
                let mid_x = bounds.origin.x + bounds.size.width * 0.5;
                let gap = px(9.0);
                let amp = px(2.4);

                if bounds.size.width > px(36.) {
                    let left_start = bounds.origin.x;
                    let left_end = mid_x - gap;
                    let lw = left_end - left_start;
                    let mut left = PathBuilder::stroke(px(1.0));
                    left.move_to(point(left_start, y));
                    left.cubic_bezier_to(
                        point(left_end, y),
                        point(left_start + lw * 0.36, y - amp),
                        point(left_start + lw * 0.68, y + amp),
                    );
                    if let Ok(p) = left.build() {
                        window.paint_path(p, wave_color);
                    }

                    let right_start = mid_x + gap;
                    let right_end = bounds.origin.x + bounds.size.width;
                    let rw = right_end - right_start;
                    let mut right = PathBuilder::stroke(px(1.0));
                    right.move_to(point(right_start, y));
                    right.cubic_bezier_to(
                        point(right_end, y),
                        point(right_start + rw * 0.32, y - amp),
                        point(right_start + rw * 0.64, y + amp),
                    );
                    if let Ok(p) = right.build() {
                        window.paint_path(p, wave_color);
                    }
                }

                let center = point(mid_x, y);
                let mut halo = PathBuilder::fill();
                push_circle_subpath(&mut halo, center, px(5.0));
                if let Ok(p) = halo.build() {
                    window.paint_path(p, halo_color);
                }

                let mut pearl = PathBuilder::fill();
                push_circle_subpath(&mut pearl, center, px(2.6));
                if let Ok(p) = pearl.build() {
                    window.paint_path(p, pearl_color);
                }
            },
        )
        .size_full(),
    )
}

/// Seamless translucent pane surface without structural borders or drop shadows,
/// letting neighboring sections flow together as one unified glass pane.
pub fn floating_panel(id: impl Into<ElementId>, bg: Hsla, _cx: &App) -> Stateful<Div> {
    v_flex()
        .id(id)
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .bg(bg)
}

/// Borderless frosted tonal card with soft `radius_lg` rounding.
pub fn soft_card(id: impl Into<ElementId>, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let accents = ThemeAccents::from_theme(theme);
    v_flex()
        .id(id)
        .relative()
        .overflow_hidden()
        .rounded(theme.radius_lg)
        .bg(accents.surface_card)
}

/// Soft borderless circular medallion cradling a themed vector icon.
pub fn icon_medallion(icon: LucideIconName, color: Hsla, size_px: f32) -> impl IntoElement {
    let glyph = if size_px >= 44. {
        Icon::new(icon).size_5()
    } else if size_px >= 28. {
        Icon::new(icon).small()
    } else {
        Icon::new(icon).xsmall()
    };

    div()
        .size(px(size_px))
        .flex_shrink_0()
        .rounded_full()
        .bg(color.opacity(0.18))
        .flex()
        .items_center()
        .justify_center()
        .child(glyph.text_color(color))
}

/// Invisible-at-rest resize handle that reveals a centered glowing pill grabber
/// only when hovered or dragged, eliminating static seams between split sections.
pub fn soft_resize_handle() -> ResizeHandleRenderer {
    Rc::new(|handle, _window, cx| {
        let axis = handle.axis();
        let (length, pill_color) = match handle.state() {
            ResizeHandleState::Idle => (px(0.), cx.theme().transparent),
            ResizeHandleState::Hovered => (px(34.), cx.theme().primary.opacity(0.65)),
            ResizeHandleState::Pressed | ResizeHandleState::Dragging => {
                (px(52.), cx.theme().primary)
            }
        };
        let thickness = px(4.);
        let offset = (thickness - px(1.)) * -0.5;

        Some(
            div()
                .flex_none()
                .flex()
                .map(|track| match axis {
                    Axis::Horizontal => track.w(px(1.)).h_full().items_center(),
                    Axis::Vertical => track.h(px(1.)).w_full().items_start().justify_center(),
                })
                .when(length > px(0.5), |track| {
                    track.child(
                        div()
                            .flex_none()
                            .rounded_full()
                            .bg(pill_color)
                            .map(|pill| match axis {
                                Axis::Horizontal => pill.w(thickness).h(length).ml(offset),
                                Axis::Vertical => pill.h(thickness).w(length).mt(offset),
                            }),
                    )
                })
                .into_any_element(),
        )
    })
}

/// Delicate circular divider dot for the bottom status bar.
pub fn status_dot(cx: &App) -> impl IntoElement {
    div()
        .size(px(3.))
        .rounded_full()
        .flex_shrink_0()
        .bg(cx.theme().muted_foreground.opacity(0.45))
}

/// Proportional 4px pill horizontal storage micro-bar rendered inside table cells.
pub fn space_micro_bar(bytes: u64, max_bytes: u64, cx: &App) -> impl IntoElement {
    let ratio = if max_bytes == 0 {
        0.0
    } else {
        (bytes as f32 / max_bytes as f32).clamp(0.04, 1.0)
    };
    let fill_color = if ratio >= 0.65 {
        cx.theme().warning
    } else {
        cx.theme().info
    };

    div()
        .w_full()
        .h(px(4.))
        .rounded_full()
        .bg(cx.theme().muted)
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(gpui_kit::relative(ratio))
                .rounded_full()
                .bg(fill_color),
        )
}

/// Platform-aware keyboard shortcut badge rendered via `gpui_kit::component::Kbd`.
pub fn shortcut_kbd(mac_stroke: &'static str, other_stroke: &'static str, cx: &App) -> AnyElement {
    let raw = if cx.platform_info().is_mac() {
        mac_stroke
    } else {
        other_stroke
    };
    match Keystroke::parse(raw) {
        Ok(stroke) => Kbd::new(stroke).into_any_element(),
        Err(_) => div().into_any_element(),
    }
}

/// Inline normal-flow numeric count pill (no absolute overlay or negative edge offsets)
/// for sidebar items and section headers.
pub fn count_pill(
    id: impl Into<ElementId>,
    count: usize,
    max: usize,
    active: bool,
    cx: &App,
) -> AnyElement {
    let text: SharedString = if count > max {
        format!("{max}+").into()
    } else {
        count.to_string().into()
    };
    let theme = cx.theme();
    let accents = ThemeAccents::from_theme(theme);
    let (bg, fg) = if active {
        (theme.primary.opacity(0.22), theme.sidebar_accent_foreground)
    } else if count == 0 {
        (accents.sunken_bg, theme.muted_foreground.opacity(0.70))
    } else {
        (accents.sunken_bg, theme.foreground.opacity(0.88))
    };

    h_flex()
        .id(id)
        .test_support()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .h(px(18.))
        .min_w(px(22.))
        .px_1p5()
        .rounded_full()
        .bg(bg)
        .text_size(px(11.))
        .line_height(gpui_kit::relative(1.))
        .font_family(theme.mono_font_family.clone())
        .font_medium()
        .whitespace_nowrap()
        .text_color(fg)
        .child(text)
        .into_any_element()
}

/// Structured pill shimmer rows for table and file-tree loading states.
pub fn skeleton_rows(count: usize) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2p5()
        .p_3()
        .children((0..count).map(|ix| {
            let w_pct = match ix % 3 {
                0 => 0.92,
                1 => 0.78,
                _ => 0.85,
            };
            h_flex()
                .w_full()
                .gap_3()
                .items_center()
                .child(Skeleton::new().w(px(52.)).h(px(14.)).rounded_full())
                .child(Skeleton::new().w(px(72.)).h(px(14.)).rounded_full())
                .child(
                    Skeleton::new()
                        .w(gpui_kit::relative(w_pct * 0.55))
                        .h(px(14.))
                        .rounded_full(),
                )
                .child(Skeleton::new().w(px(64.)).h(px(14.)).rounded_full().secondary())
        }))
}

/// Structured pill shimmer lines matching unified diff code blocks.
pub fn skeleton_diff_lines(count: usize) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_2()
        .p_3()
        .children((0..count).map(|ix| {
            let width = match ix % 4 {
                0 => 0.86,
                1 => 0.62,
                2 => 0.74,
                _ => 0.48,
            };
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .child(Skeleton::new().w(px(32.)).h(px(12.)).rounded_full().secondary())
                .child(
                    Skeleton::new()
                        .w(gpui_kit::relative(width))
                        .h(px(12.))
                        .rounded_full(),
                )
        }))
}

/// Structured rounded shimmer cards matching Settings configuration panels.
pub fn skeleton_settings_cards(count: usize) -> impl IntoElement {
    v_flex()
        .w_full()
        .gap_4()
        .children((0..count).map(|_| {
            v_flex()
                .w_full()
                .p_4()
                .gap_3()
                .rounded(px(16.))
                .child(Skeleton::new().w(px(180.)).h(px(16.)).rounded_full())
                .child(Skeleton::new().w_full().h(px(30.)).rounded(px(10.)).secondary())
                .child(Skeleton::new().w_full().h(px(30.)).rounded(px(10.)).secondary())
        }))
}

/// Soft frosted empty-state card with a circular icon medallion, clear guidance copy,
/// and an optional recovery action.
pub fn empty_state_card(
    id: impl Into<ElementId>,
    icon: LucideIconName,
    icon_color: Hsla,
    title: impl Into<gpui_kit::SharedString>,
    subtitle: impl Into<gpui_kit::SharedString>,
    action: Option<AnyElement>,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();

    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_6()
        .child(
            soft_card(id, cx)
                .max_w(px(440.))
                .w_full()
                .p_6()
                .gap_3()
                .items_center()
                .child(icon_medallion(icon, icon_color, 52.))
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(theme.foreground)
                        .text_center()
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .text_center()
                        .child(subtitle.into()),
                )
                .when_some(action, |this, el| this.child(div().pt_1().child(el))),
        )
}

#[cfg(test)]
mod tests {
    use gpui_kit::AssetSource as _;
    use gpui_kit::assets::{AllAssets, IconName as LucideIconName};

    #[test]
    fn all_snapperman_lucide_icons_load_from_all_assets() {
        let icons = [
            LucideIconName::Activity,
            LucideIconName::Archive,
            LucideIconName::ArrowRight,
            LucideIconName::ChartPie,
            LucideIconName::ChevronRight,
            LucideIconName::Clock,
            LucideIconName::Compass,
            LucideIconName::Cpu,
            LucideIconName::FileDiff,
            LucideIconName::Flame,
            LucideIconName::FolderTree,
            LucideIconName::Gauge,
            LucideIconName::GitCompareArrows,
            LucideIconName::HardDrive,
            LucideIconName::Inbox,
            LucideIconName::KeyRound,
            LucideIconName::Layers,
            LucideIconName::Lock,
            LucideIconName::LockOpen,
            LucideIconName::Moon,
            LucideIconName::Palette,
            LucideIconName::Pin,
            LucideIconName::PinOff,
            LucideIconName::Plus,
            LucideIconName::RotateCcw,
            LucideIconName::Search,
            LucideIconName::Settings2,
            LucideIconName::ShieldAlert,
            LucideIconName::ShieldCheck,
            LucideIconName::SlidersHorizontal,
            LucideIconName::Sparkles,
            LucideIconName::Sun,
            LucideIconName::TriangleAlert,
            LucideIconName::X,
            LucideIconName::Zap,
        ];

        for icon in icons {
            let path = icon.path();
            let loaded = AllAssets
                .load(path.as_ref())
                .unwrap_or_else(|e| panic!("failed to load {path}: {e}"));
            let bytes = loaded.unwrap_or_else(|| panic!("missing SVG bytes for {path}"));
            assert!(!bytes.is_empty(), "empty SVG asset for {path}");
        }
    }
}
