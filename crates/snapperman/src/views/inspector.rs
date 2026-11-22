use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::description_list::{DescriptionItem, DescriptionList};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use snapper_core::format::format_size;
use snapper_core::model::SnapshotKey;

use crate::state::{InspectorTab, WorkspaceEvent, WorkspaceModel};
use crate::theme::ThemeAccents;
use crate::views::aether;
use crate::views::dialogs::compare::open_compare_dialog;
use crate::views::dialogs::confirm::{open_delete_confirm_dialog, open_rollback_confirm_dialog};
use crate::views::dialogs::modify::open_modify_snapshot_dialog;
use crate::views::diff_pane::DiffPaneView;

pub struct InspectorView {
    workspace: Entity<WorkspaceModel>,
    diff_pane: Entity<DiffPaneView>,
    _sub: Subscription,
}

impl InspectorView {
    pub fn new(
        workspace: Entity<WorkspaceModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ws = workspace.clone();
        let diff_pane = cx.new(|cx| DiffPaneView::new(ws, window, cx));
        let sub = cx.subscribe(&workspace, |_, _, event: &WorkspaceEvent, cx| {
            if matches!(
                event,
                WorkspaceEvent::SnapshotsChanged
                    | WorkspaceEvent::SelectionChanged
                    | WorkspaceEvent::StatusChanged
            ) {
                cx.notify();
            }
        });

        Self {
            workspace,
            diff_pane,
            _sub: sub,
        }
    }

    pub fn diff_pane(&self) -> &Entity<DiffPaneView> {
        &self.diff_pane
    }
}

fn telemetry_cell(
    icon: LucideIconName,
    icon_color: gpui_kit::Hsla,
    label: &'static str,
    value_el: impl IntoElement,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    let accents = ThemeAccents::from_theme(cx.theme());
    v_flex()
        .flex_1()
        .min_w(px(140.))
        .p_3()
        .gap_1p5()
        .rounded(px(12.))
        .bg(accents.sunken_bg)
        .child(
            h_flex()
                .gap_1p5()
                .items_center()
                .child(aether::icon_medallion(icon, icon_color, 22.))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                ),
        )
        .child(value_el)
}

impl Render for InspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accents = ThemeAccents::from_theme(cx.theme());
        let ws = self.workspace.read(cx);
        let Some(snap) = ws.selected_snapshot().cloned() else {
            return aether::floating_panel("inspector-empty", accents.surface_panel, cx)
                .child(aether::empty_state_card(
                    "inspector-empty-plaque",
                    LucideIconName::Compass,
                    cx.theme().primary,
                    "No snapshot selected",
                    "Select a snapshot from the table to inspect telemetry, retention tags, or browse file diffs.",
                    None,
                    cx,
                ))
                .into_any_element();
        };

        let active_tab = ws.inspector_tab();
        let hydrating_space = ws.hydrating_space();
        let file_count = ws.file_changes().len();
        let status_loading = ws.status_loading();
        let key = snap.key();
        let is_pinned = snap.is_pinned();
        let is_protected = snap.is_protected();
        let is_zero = snap.number == 0;

        let changes_label: SharedString = if status_loading {
            "Changes (…)" .into()
        } else if file_count > 0 {
            format!("Changes ({file_count})").into()
        } else {
            "Changes".into()
        };

        let rail_color = if snap.active {
            cx.theme().success
        } else if snap.number == 0 || snap.default {
            cx.theme().info
        } else {
            cx.theme().primary
        };

        let desc_preview = if snap.description.is_empty() {
            if is_zero {
                "Live filesystem subvolume".to_string()
            } else {
                "No snapshot description".to_string()
            }
        } else {
            snap.description.clone()
        };

        let ws_tab = self.workspace.clone();
        let ws_pin = self.workspace.clone();
        let ws_mod = self.workspace.clone();
        let ws_cmp = self.workspace.clone();
        let ws_rb = self.workspace.clone();
        let ws_del = self.workspace.clone();

        let snap_mod = snap.clone();
        let snap_cmp = snap.clone();
        let snap_rb = snap.clone();
        let snap_del = snap.clone();

        aether::floating_panel("inspector-panel", accents.surface_panel, cx)
            .child(
                v_flex()
                    .w_full()
                    .px_3p5()
                    .pt_3()
                    .pb_2p5()
                    .gap_2()
                    .bg(accents.header_tint)
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .flex_wrap()
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        div()
                                            .size(px(10.))
                                            .rounded_full()
                                            .bg(rail_color.opacity(0.24))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(div().size(px(6.)).rounded_full().bg(rail_color)),
                                    )
                                    .child(
                                        div()
                                            .font_family(cx.theme().mono_font_family.clone())
                                            .text_base()
                                            .font_bold()
                                            .text_color(rail_color)
                                            .child(format!("#{}", key.number)),
                                    )
                                    .child(
                                        Tag::secondary()
                                            .xsmall()
                                            .rounded_full()
                                            .border_0()
                                            .child(key.config.clone()),
                                    )
                                    .when(snap.active, |this| {
                                        this.child(
                                            Tag::success()
                                                .xsmall()
                                                .rounded_full()
                                                .border_0()
                                                .child("active"),
                                        )
                                    })
                                    .when(snap.default, |this| {
                                        this.child(
                                            Tag::info()
                                                .xsmall()
                                                .rounded_full()
                                                .border_0()
                                                .child("default"),
                                        )
                                    })
                                    .when(is_pinned, |this| {
                                        this.child(
                                            Tag::warning()
                                                .xsmall()
                                                .rounded_full()
                                                .border_0()
                                                .child("pinned"),
                                        )
                                    })
                                    .when(snap.is_important(), |this| {
                                        this.child(
                                            Tag::primary()
                                                .xsmall()
                                                .rounded_full()
                                                .border_0()
                                                .child("important"),
                                        )
                                    }),
                            )
                            .child(
                                TabBar::new("inspector-tabs")
                                    .pill()
                                    .small()
                                    .selected_index(match active_tab {
                                        InspectorTab::Overview => 0,
                                        InspectorTab::Changes => 1,
                                    })
                                    .on_click(move |ix, _, cx| {
                                        ws_tab.update(cx, |ws, cx| {
                                            ws.set_inspector_tab(
                                                if *ix == 1 {
                                                    InspectorTab::Changes
                                                } else {
                                                    InspectorTab::Overview
                                                },
                                                cx,
                                            );
                                        });
                                    })
                                    .child(
                                        Tab::new().child(
                                            h_flex()
                                                .id("inspector-tab-overview")
                                                .test_support()
                                                .gap_1()
                                                .items_center()
                                                .child(Icon::new(LucideIconName::Layers).xsmall())
                                                .child("Overview"),
                                        ),
                                    )
                                    .child(
                                        Tab::new().child(
                                            h_flex()
                                                .id("inspector-tab-changes")
                                                .test_support()
                                                .gap_1()
                                                .items_center()
                                                .child(Icon::new(LucideIconName::FileDiff).xsmall())
                                                .child(changes_label),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(desc_preview),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .justify_between()
                            .gap_1p5()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        Button::new("inspector-pin")
                                            .xsmall()
                                            .secondary()
                                            .rounded(aether::pill_radius())
                                            .icon(if is_pinned {
                                                Icon::new(LucideIconName::PinOff)
                                            } else {
                                                Icon::new(LucideIconName::Pin)
                                            })
                                            .label(if is_pinned { "Unpin" } else { "Pin" })
                                            .on_click(move |_, _, cx| {
                                                ws_pin.update(cx, |ws, cx| ws.toggle_pin_selected(cx));
                                            }),
                                    )
                                    .child(
                                        Button::new("inspector-modify")
                                            .xsmall()
                                            .secondary()
                                            .rounded(aether::pill_radius())
                                            .icon(IconName::Settings2)
                                            .label("Modify…")
                                            .on_click(move |_, window, cx| {
                                                open_modify_snapshot_dialog(
                                                    ws_mod.clone(),
                                                    snap_mod.clone(),
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    )
                                    .child(
                                        Button::new("inspector-compare")
                                            .xsmall()
                                            .secondary()
                                            .rounded(aether::pill_radius())
                                            .icon(Icon::new(LucideIconName::GitCompareArrows))
                                            .label("Compare…")
                                            .on_click(move |_, window, cx| {
                                                open_compare_dialog(
                                                    ws_cmp.clone(),
                                                    Some((snap_cmp.config.clone(), snap_cmp.number, 0)),
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .child(
                                        Button::new("inspector-rollback")
                                            .xsmall()
                                            .warning()
                                            .rounded(aether::pill_radius())
                                            .icon(Icon::new(LucideIconName::RotateCcw))
                                            .label("Rollback…")
                                            .disabled(is_zero)
                                            .tooltip(if is_zero {
                                                "Snapshot #0 is already the live filesystem"
                                            } else {
                                                "Roll back default subvolume to this snapshot"
                                            })
                                            .on_click(move |_, window, cx| {
                                                open_rollback_confirm_dialog(
                                                    ws_rb.clone(),
                                                    snap_rb.clone(),
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    )
                                    .child(
                                        Button::new("inspector-delete")
                                            .xsmall()
                                            .danger()
                                            .rounded(aether::pill_radius())
                                            .icon(IconName::Delete)
                                            .label("Delete…")
                                            .disabled(is_protected)
                                            .tooltip(if is_protected {
                                                "Snapshot #0, active, and default snapshots are protected"
                                            } else {
                                                "Delete this snapshot"
                                            })
                                            .on_click(move |_, window, cx| {
                                                open_delete_confirm_dialog(
                                                    ws_del.clone(),
                                                    vec![snap_del.clone()],
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    ),
                            ),
                    ),
            )
            .child(match active_tab {
                InspectorTab::Overview => {
                    let pair_num = if snap.is_pre() {
                        snap.post_number
                    } else if snap.is_post() {
                        snap.pre_number
                    } else {
                        None
                    };
                    let space_text = match snap.used_space {
                        Some(bytes) => format_size(bytes),
                        None if hydrating_space => "Calculating…".to_string(),
                        None => "Unavailable".to_string(),
                    };
                    let retention_text = snap
                        .cleanup
                        .clone()
                        .unwrap_or_else(|| "pinned (manual)".to_string());
                    let subvol_value = snap.subvolume.clone();
                    let type_elem = {
                        let ws_pair = self.workspace.clone();
                        let cfg_pair = snap.config.clone();
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_xs().font_semibold().child(snap.snapshot_type.clone()))
                            .when_some(pair_num, |this, n| {
                                this.child(
                                    Button::new("inspector-jump-pair")
                                        .xsmall()
                                        .secondary()
                                        .rounded(aether::pill_radius())
                                        .icon(Icon::new(LucideIconName::GitCompareArrows))
                                        .label(format!("#{n}"))
                                        .on_click(move |_, _, cx| {
                                            ws_pair.update(cx, |ws, cx| {
                                                ws.select_key(
                                                    SnapshotKey::new(cfg_pair.clone(), n),
                                                    cx,
                                                );
                                            });
                                        }),
                                )
                            })
                            .into_any_element()
                    };
                    let subvol_elem = h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            div()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_xs()
                                .child(subvol_value.clone()),
                        )
                        .child(
                            Clipboard::new("inspector-copy-subvolume")
                                .value(subvol_value)
                                .tooltip("Copy subvolume path"),
                        )
                        .into_any_element();

                    let userdata_entries: Vec<(String, String)> = snap
                        .userdata
                        .as_ref()
                        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                        .unwrap_or_default();

                    let ws_inspect_diff = self.workspace.clone();
                    let changes_summary: SharedString = if status_loading {
                        "Scanning…".into()
                    } else if file_count > 0 {
                        format!("{file_count} changed").into()
                    } else {
                        "Click Inspect".into()
                    };

                    div()
                        .id("inspector-overview-scroll")
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .p_3p5()
                        .overflow_y_scrollbar()
                        .child(
                            v_flex()
                                .gap_3()
                                .when(is_protected, |this| {
                                    this.child(
                                        div()
                                            .id("inspector-protected-note")
                                            .test_support()
                                            .w_full()
                                            .child(
                                                Alert::info(
                                                    "inspector-protected-alert",
                                                    "Protected snapshot (#0 / active / default): deletion is disabled to keep the live system consistent.",
                                                )
                                                .icon(Icon::new(LucideIconName::ShieldCheck))
                                                .small(),
                                            ),
                                    )
                                })
                                .child(
                                    v_flex()
                                        .w_full()
                                        .gap_2()
                                        .child(
                                            h_flex()
                                                .w_full()
                                                .gap_2()
                                                .child(telemetry_cell(
                                                    LucideIconName::HardDrive,
                                                    cx.theme().info,
                                                    "Used Space",
                                                    div()
                                                        .font_family(cx.theme().mono_font_family.clone())
                                                        .text_sm()
                                                        .font_bold()
                                                        .text_color(cx.theme().foreground)
                                                        .child(space_text.clone()),
                                                    cx,
                                                ))
                                                .child(telemetry_cell(
                                                    LucideIconName::Clock,
                                                    cx.theme().primary,
                                                    "Retention Policy",
                                                    div()
                                                        .font_family(cx.theme().mono_font_family.clone())
                                                        .text_sm()
                                                        .font_semibold()
                                                        .text_color(cx.theme().foreground)
                                                        .child(retention_text),
                                                    cx,
                                                )),
                                        )
                                        .child(
                                            h_flex()
                                                .w_full()
                                                .gap_2()
                                                .child(telemetry_cell(
                                                    LucideIconName::Layers,
                                                    cx.theme().warning,
                                                    "Snapshot Type",
                                                    type_elem,
                                                    cx,
                                                ))
                                                .child(telemetry_cell(
                                                    LucideIconName::FileDiff,
                                                    cx.theme().success,
                                                    "File Delta",
                                                    h_flex()
                                                        .w_full()
                                                        .justify_between()
                                                        .items_center()
                                                        .gap_1()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_semibold()
                                                                .text_color(cx.theme().foreground)
                                                                .child(changes_summary),
                                                        )
                                                        .child(
                                                            Button::new("inspector-quick-changes")
                                                                .xsmall()
                                                                .secondary()
                                                                .rounded(aether::pill_radius())
                                                                .icon(Icon::new(LucideIconName::ArrowRight))
                                                                .label("Diffs")
                                                                .on_click(move |_, _, cx| {
                                                                    ws_inspect_diff.update(cx, |ws, cx| {
                                                                        ws.set_inspector_tab(
                                                                            InspectorTab::Changes,
                                                                            cx,
                                                                        );
                                                                    });
                                                                }),
                                                        ),
                                                    cx,
                                                )),
                                        ),
                                )
                                .child(aether::filigree_wave(cx))
                                .child(
                                    div()
                                        .w_full()
                                        .p_3()
                                        .rounded(px(12.))
                                        .bg(accents.sunken_bg)
                                        .child(
                                            DescriptionList::horizontal()
                                                .bordered(false)
                                                .columns(1)
                                                .small()
                                                .label_width(px(110.))
                                                .child(
                                                    DescriptionItem::new("Number")
                                                        .value(format!("#{}", snap.number)),
                                                )
                                                .child(
                                                    DescriptionItem::new("Config")
                                                        .value(snap.config.clone()),
                                                )
                                                .child(
                                                    DescriptionItem::new("Created")
                                                        .value(snap.date.clone()),
                                                )
                                                .child(
                                                    DescriptionItem::new("User")
                                                        .value(snap.user.clone()),
                                                )
                                                .child(
                                                    DescriptionItem::new("Subvolume")
                                                        .value(subvol_elem),
                                                )
                                                .child(
                                                    DescriptionItem::new("Cleanup").value(
                                                        snap.cleanup
                                                            .clone()
                                                            .unwrap_or_else(|| "none".to_string()),
                                                    ),
                                                )
                                                .child(
                                                    DescriptionItem::new("Used Space")
                                                        .value(space_text),
                                                )
                                                .child(
                                                    DescriptionItem::new("Description").value(
                                                        if snap.description.is_empty() {
                                                            "—".to_string()
                                                        } else {
                                                            snap.description.clone()
                                                        },
                                                    ),
                                                ),
                                        ),
                                )
                                .when(!userdata_entries.is_empty(), |this| {
                                    this.child(
                                        v_flex()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Userdata tags"),
                                            )
                                            .child(
                                                h_flex().flex_wrap().gap_1p5().children(
                                                    userdata_entries.into_iter().map(|(k, v)| {
                                                        Tag::secondary()
                                                            .small()
                                                            .rounded_full()
                                                            .border_0()
                                                            .child(format!("{k}={v}"))
                                                    }),
                                                ),
                                            ),
                                    )
                                }),
                        )
                        .into_any_element()
                }
                InspectorTab::Changes => div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .w_full()
                    .child(self.diff_pane.clone())
                    .into_any_element(),
            })
            .into_any_element()
    }
}
