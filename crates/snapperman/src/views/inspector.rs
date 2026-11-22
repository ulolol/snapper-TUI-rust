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

impl Render for InspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let Some(snap) = ws.selected_snapshot().cloned() else {
            return v_flex()
                .id("inspector-empty")
                .size_full()
                .min_w_0()
                .min_h_0()
                .items_center()
                .justify_center()
                .gap_2()
                .p_6()
                .bg(cx.theme().background)
                .child(
                    Icon::new(IconName::Inspector)
                        .size_10()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(cx.theme().foreground)
                        .child("No snapshot selected"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Select a snapshot from the table to inspect details or browse file diffs."),
                )
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

        v_flex()
            .id("inspector-panel")
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w_full()
                    .p_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .font_family(cx.theme().mono_font_family.clone())
                                            .text_base()
                                            .font_semibold()
                                            .text_color(cx.theme().foreground)
                                            .child(format!("#{} ({})", key.number, key.config)),
                                    )
                                    .when(snap.active, |this| {
                                        this.child(Tag::success().xsmall().outline().child("active"))
                                    })
                                    .when(snap.default, |this| {
                                        this.child(Tag::info().xsmall().outline().child("default"))
                                    })
                                    .when(is_pinned, |this| {
                                        this.child(Tag::warning().xsmall().outline().child("pinned"))
                                    })
                                    .when(snap.is_important(), |this| {
                                        this.child(Tag::primary().xsmall().outline().child("important"))
                                    }),
                            )
                            .child(
                                TabBar::new("inspector-tabs")
                                    .segmented()
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
                                            div()
                                                .id("inspector-tab-overview")
                                                .test_support()
                                                .child("Overview"),
                                        ),
                                    )
                                    .child(
                                        Tab::new().child(
                                            div()
                                                .id("inspector-tab-changes")
                                                .test_support()
                                                .child(changes_label),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_1()
                            .items_center()
                            .child(
                                Button::new("inspector-pin")
                                    .xsmall()
                                    .outline()
                                    .icon(if is_pinned {
                                        IconName::StarOff
                                    } else {
                                        IconName::Star
                                    })
                                    .label(if is_pinned { "Unpin" } else { "Pin" })
                                    .on_click(move |_, _, cx| {
                                        ws_pin.update(cx, |ws, cx| ws.toggle_pin_selected(cx));
                                    }),
                            )
                            .child(
                                Button::new("inspector-modify")
                                    .xsmall()
                                    .outline()
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
                                    .outline()
                                    .icon(IconName::Replace)
                                    .label("Compare…")
                                    .on_click(move |_, window, cx| {
                                        open_compare_dialog(
                                            ws_cmp.clone(),
                                            Some((snap_cmp.config.clone(), snap_cmp.number, 0)),
                                            window,
                                            cx,
                                        );
                                    }),
                            )
                            .child(
                                Button::new("inspector-rollback")
                                    .xsmall()
                                    .outline()
                                    .icon(IconName::Undo2)
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
                    let subvol_value = snap.subvolume.clone();
                    let type_elem = {
                        let ws_pair = self.workspace.clone();
                        let cfg_pair = snap.config.clone();
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().child(snap.snapshot_type.clone()))
                            .when_some(pair_num, |this, n| {
                                this.child(
                                    Button::new("inspector-jump-pair")
                                        .xsmall()
                                        .outline()
                                        .label(format!("Jump to #{n}"))
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

                    div()
                        .id("inspector-overview-scroll")
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .p_3()
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
                                                .small(),
                                            ),
                                    )
                                })
                                .child(
                                    DescriptionList::horizontal()
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
                                            DescriptionItem::new("Type").value(type_elem),
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
                                )
                                .when(!userdata_entries.is_empty(), |this| {
                                    this.child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("USERDATA"),
                                            )
                                            .child(
                                                h_flex().flex_wrap().gap_1().children(
                                                    userdata_entries.into_iter().map(|(k, v)| {
                                                        Tag::secondary()
                                                            .small()
                                                            .outline()
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
