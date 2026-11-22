use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::chart::BarChart;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _,
    Window, div, px,
};
use snapper_core::format::format_size;
use snapper_core::model::{Snapshot, SnapshotKey};
use snapper_core::query::compute_storage_summary;

use crate::state::{ActiveView, WorkspaceEvent, WorkspaceModel};
use crate::views::aether;

pub struct AnalyticsView {
    workspace: Entity<WorkspaceModel>,
    _sub: Subscription,
}

impl AnalyticsView {
    pub fn new(workspace: Entity<WorkspaceModel>, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&workspace, |_, _, event: &WorkspaceEvent, cx| {
            if matches!(event, WorkspaceEvent::SnapshotsChanged) {
                cx.notify();
            }
        });
        Self {
            workspace,
            _sub: sub,
        }
    }
}

fn kpi_card(
    id: &'static str,
    icon: LucideIconName,
    title: &'static str,
    value: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    accent: gpui_kit::Hsla,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    aether::soft_card(id, cx)
        .flex_1()
        .min_w(px(175.))
        .child(
            v_flex()
                .p_4()
                .gap_2()
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_semibold()
                                .text_color(cx.theme().muted_foreground)
                                .child(title),
                        )
                        .child(aether::icon_medallion(icon, accent, 28.)),
                )
                .child(
                    div()
                        .text_xl()
                        .font_bold()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_color(accent)
                        .child(value.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(subtitle.into()),
                ),
        )
}

impl Render for AnalyticsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let summary = compute_storage_summary(ws.snapshots());
        let hydrating = ws.hydrating_space();

        let mut sized_snaps: Vec<Snapshot> = ws
            .snapshots()
            .iter()
            .filter(|s| s.used_space.is_some())
            .cloned()
            .collect();
        let sized_count = sized_snaps.len();
        sized_snaps.sort_by(|a, b| {
            b.used_space
                .unwrap_or(0)
                .cmp(&a.used_space.unwrap_or(0))
                .then_with(|| b.number.cmp(&a.number))
        });

        let snap_chart_data: Vec<Snapshot> = sized_snaps.iter().take(10).cloned().collect();
        let cfg_chart_data = summary.per_config.clone();
        let top_rows: Vec<Snapshot> = sized_snaps.into_iter().take(15).collect();

        let largest_value: SharedString = if summary.largest_snapshot.is_some() {
            format_size(summary.largest_snapshot_bytes).into()
        } else {
            "—".into()
        };
        let largest_sub: SharedString = summary
            .largest_snapshot
            .as_ref()
            .map(|k| format!("{} (#{})", k.config, k.number))
            .unwrap_or_else(|| "No sized snapshot".to_string())
            .into();

        let chart1 = cx.theme().chart_1;
        let chart2 = cx.theme().chart_2;

        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .id("analytics-view")
                    .test_support()
                    .w_full()
                    .p_5()
                    .gap_4()
                    .child(
                        aether::soft_card("analytics-header-card", cx)
                            .w_full()
                            .child(
                                h_flex()
                                    .w_full()
                                    .px_4()
                                    .py_3()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2p5()
                                            .items_center()
                                            .child(aether::brand_sigil(px(24.), cx))
                                            .child(
                                                v_flex()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_lg()
                                                            .font_bold()
                                                            .text_color(cx.theme().foreground)
                                                            .child("Storage & Snapshot Analytics"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child("Exclusive Btrfs subvolume usage aggregated across loaded Snapper configs."),
                                                    ),
                                            ),
                                    )
                                    .child(if hydrating {
                                        Tag::info()
                                            .small()
                                            .rounded_full()
                                            .border_0()
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .items_center()
                                                    .child(Icon::new(LucideIconName::Activity).xsmall())
                                                    .child("Calculating used-space…"),
                                            )
                                            .into_any_element()
                                    } else {
                                        Tag::success()
                                            .small()
                                            .rounded_full()
                                            .border_0()
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .items_center()
                                                    .child(Icon::new(LucideIconName::ShieldCheck).xsmall())
                                                    .child(format!(
                                                        "{}/{} snapshots sized",
                                                        sized_count, summary.total_snapshots
                                                    )),
                                            )
                                            .into_any_element()
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_3()
                            .child(kpi_card(
                                "kpi-total-space",
                                LucideIconName::Gauge,
                                "Total Tracked Space",
                                format_size(summary.total_used_bytes),
                                "Sum of reported used-space",
                                cx.theme().primary,
                                cx,
                            ))
                            .child(kpi_card(
                                "kpi-snapshot-count",
                                LucideIconName::Layers,
                                "Total Snapshots",
                                summary.total_snapshots.to_string(),
                                format!("Across {} configuration(s)", summary.per_config.len()),
                                cx.theme().info,
                                cx,
                            ))
                            .child(kpi_card(
                                "kpi-configs",
                                LucideIconName::HardDrive,
                                "Configurations",
                                summary.per_config.len().to_string(),
                                "Active Snapper profiles",
                                cx.theme().success,
                                cx,
                            ))
                            .child(kpi_card(
                                "kpi-largest",
                                LucideIconName::Flame,
                                "Largest Snapshot",
                                largest_value,
                                largest_sub,
                                cx.theme().warning,
                                cx,
                            )),
                    )
                    .child(aether::filigree_wave(cx))
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_4()
                            .child(
                                aether::soft_card("analytics-card-top-snapshots", cx)
                                    .flex_1()
                                    .min_w(px(340.))
                                    .child(
                                        v_flex()
                                            .p_4()
                                            .gap_3()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(aether::icon_medallion(
                                                        LucideIconName::Layers,
                                                        cx.theme().chart_1,
                                                        26.,
                                                    ))
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_semibold()
                                                            .text_color(cx.theme().foreground)
                                                            .child("Top Snapshots by Used Space"),
                                                    ),
                                            )
                                            .child(
                                                div().w_full().h(px(240.)).child(
                                                    BarChart::new(snap_chart_data)
                                                        .id("analytics-top-snapshots-chart")
                                                        .band(|d| format!("{}:#{}", d.config, d.number))
                                                        .value(|d| d.used_space.unwrap_or(0) as f64)
                                                        .fill(move |_, _, _, _| {
                                                            gpui_kit::Background::from(chart1)
                                                        })
                                                        .value_axis(true)
                                                        .value_tick_format(|v| {
                                                            if v <= 0.0 {
                                                                "0".to_string()
                                                            } else {
                                                                format_size(v as u64)
                                                            }
                                                        })
                                                        .tooltip_title(|d| {
                                                            format!("{} #{}", d.config, d.number).into()
                                                        })
                                                        .tooltip_value(|_, v| {
                                                            format_size(v as u64).into()
                                                        }),
                                                ),
                                            ),
                                    ),
                            )
                            .child(
                                aether::soft_card("analytics-card-by-config", cx)
                                    .flex_1()
                                    .min_w(px(300.))
                                    .child(
                                        v_flex()
                                            .p_4()
                                            .gap_3()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(aether::icon_medallion(
                                                        LucideIconName::ChartPie,
                                                        cx.theme().chart_2,
                                                        26.,
                                                    ))
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_semibold()
                                                            .text_color(cx.theme().foreground)
                                                            .child("Storage Breakdown by Config"),
                                                    ),
                                            )
                                            .child(
                                                div().w_full().h(px(240.)).child(
                                                    BarChart::new(cfg_chart_data)
                                                        .id("analytics-by-config-chart")
                                                        .band(|d| d.config.clone())
                                                        .value(|d| d.total_bytes as f64)
                                                        .fill(move |_, _, _, _| {
                                                            gpui_kit::Background::from(chart2)
                                                        })
                                                        .value_axis(true)
                                                        .value_tick_format(|v| {
                                                            if v <= 0.0 {
                                                                "0".to_string()
                                                            } else {
                                                                format_size(v as u64)
                                                            }
                                                        })
                                                        .tooltip_title(|d| {
                                                            format!(
                                                                "{} ({} snaps)",
                                                                d.config, d.snapshot_count
                                                            )
                                                            .into()
                                                        })
                                                        .tooltip_value(|_, v| {
                                                            format_size(v as u64).into()
                                                        }),
                                                ),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        aether::soft_card("analytics-heaviest-card", cx)
                            .w_full()
                            .child(
                                v_flex()
                                    .p_4()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(aether::icon_medallion(
                                                        LucideIconName::Flame,
                                                        cx.theme().warning,
                                                        26.,
                                                    ))
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_semibold()
                                                            .text_color(cx.theme().foreground)
                                                            .child("Heaviest Snapshots (Click to Inspect)"),
                                                    ),
                                            )
                                            .child(aether::count_pill(
                                                "analytics-top-count",
                                                top_rows.len(),
                                                999,
                                                false,
                                                cx,
                                            )),
                                    )
                                    .children(top_rows.into_iter().enumerate().map(|(ix, entry)| {
                                        let ws_jump = self.workspace.clone();
                                        let ws_btn = self.workspace.clone();
                                        let key =
                                            SnapshotKey::new(entry.config.clone(), entry.number);
                                        let key_btn = key.clone();
                                        let bytes = entry.used_space.unwrap_or(0);
                                        let share_pct = if summary.total_used_bytes > 0 {
                                            (bytes as f32 / summary.total_used_bytes as f32
                                                * 100.0)
                                                .clamp(0.0, 100.0)
                                        } else {
                                            0.0
                                        };
                                        let row_id: SharedString =
                                            format!("analytics-top-row-{ix}").into();
                                        let bar_id: SharedString =
                                            format!("analytics-share-{ix}").into();

                                        h_flex()
                                            .id(row_id)
                                            .w_full()
                                            .px_3()
                                            .py_2()
                                            .gap_3()
                                            .items_center()
                                            .justify_between()
                                            .rounded(cx.theme().radius)
                                            .hover(|s| s.bg(cx.theme().list_hover))
                                            .on_click(move |_, _, cx| {
                                                ws_jump.update(cx, |ws, cx| {
                                                    ws.set_active_view(ActiveView::Snapshots, cx);
                                                    ws.set_config_filter(None, cx);
                                                    ws.select_key(key.clone(), cx);
                                                });
                                            })
                                            .child(
                                                h_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .font_family(
                                                                cx.theme().mono_font_family.clone(),
                                                            )
                                                            .text_xs()
                                                            .font_semibold()
                                                            .text_color(cx.theme().info)
                                                            .child(format!("#{}", entry.number)),
                                                    )
                                                    .child(
                                                        Tag::secondary()
                                                            .xsmall()
                                                            .rounded_full()
                                                            .border_0()
                                                            .child(entry.config.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .text_sm()
                                                            .truncate()
                                                            .child(if entry.description.is_empty() {
                                                                "—".to_string()
                                                            } else {
                                                                entry.description.clone()
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_3()
                                                    .items_center()
                                                    .child(
                                                        h_flex()
                                                            .gap_1p5()
                                                            .items_center()
                                                            .child(
                                                                Progress::new(bar_id)
                                                                    .xsmall()
                                                                    .value(share_pct)
                                                                    .w(px(72.)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .w(px(46.))
                                                                    .text_right()
                                                                    .font_family(
                                                                        cx.theme()
                                                                            .mono_font_family
                                                                            .clone(),
                                                                    )
                                                                    .text_xs()
                                                                    .text_color(
                                                                        cx.theme().muted_foreground,
                                                                    )
                                                                    .child(format!(
                                                                        "{share_pct:.1}%"
                                                                    )),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(72.))
                                                            .text_right()
                                                            .font_family(
                                                                cx.theme().mono_font_family.clone(),
                                                            )
                                                            .text_xs()
                                                            .font_semibold()
                                                            .child(format_size(bytes)),
                                                    )
                                                    .child(
                                                        Button::new(format!(
                                                            "analytics-inspect-{ix}"
                                                        ))
                                                        .xsmall()
                                                        .secondary()
                                                        .rounded(aether::pill_radius())
                                                        .icon(Icon::new(LucideIconName::ArrowRight))
                                                        .label("Inspect")
                                                        .on_click(move |_, _, cx| {
                                                            ws_btn.update(cx, |ws, cx| {
                                                                ws.set_active_view(
                                                                    ActiveView::Snapshots,
                                                                    cx,
                                                                );
                                                                ws.set_config_filter(None, cx);
                                                                ws.select_key(key_btn.clone(), cx);
                                                            });
                                                        }),
                                                    ),
                                            )
                                    })),
                            ),
                    ),
            )
    }
}
