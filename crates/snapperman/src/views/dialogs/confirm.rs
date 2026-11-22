use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_kit::component::form::{field, v_form};
use gpui_kit::component::select::{Select, SelectState};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Entity, Focusable as _, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use snapper_core::model::{CleanupAlgorithm, Snapshot, SnapshotKey};

use crate::state::WorkspaceModel;
use crate::views::aether;
use crate::views::dialogs::{ChoiceItem, dialog_header_banner};

pub fn open_delete_confirm_dialog(
    workspace: Entity<WorkspaceModel>,
    targets: Vec<Snapshot>,
    window: &mut Window,
    cx: &mut App,
) {
    let protected: Vec<String> = targets
        .iter()
        .filter(|s| s.is_protected())
        .map(|s| s.key().to_string())
        .collect();
    let deletable: Vec<SnapshotKey> = targets
        .iter()
        .filter(|s| !s.is_protected())
        .map(|s| s.key())
        .collect();

    if deletable.is_empty() {
        window.open_alert_dialog(cx, move |alert, _, cx| {
            let list = if protected.is_empty() {
                "No deletable snapshot selected.".to_string()
            } else {
                format!(
                    "Protected snapshot(s) cannot be deleted: {}",
                    protected.join(", ")
                )
            };
            alert
                .title("Protected Snapshot")
                .description(list)
                .icon(Icon::new(IconName::Info).text_color(cx.theme().info))
                .ok_text("Close")
        });
        return;
    }

    let summary_lines: Vec<String> = deletable
        .iter()
        .take(6)
        .map(|k| format!("  • {} (#{})", k.config, k.number))
        .collect();
    let more = if deletable.len() > 6 {
        format!("\n  …and {} more", deletable.len() - 6)
    } else {
        String::new()
    };
    let protected_note = if protected.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nSkipped protected snapshot(s): {}",
            protected.join(", ")
        )
    };
    let description = format!(
        "This permanently removes {} snapshot(s):\n{}{}{}",
        deletable.len(),
        summary_lines.join("\n"),
        more,
        protected_note
    );
    let label = if deletable.len() == 1 {
        format!("Delete {}…", deletable[0])
    } else {
        format!("Delete {} Snapshots", deletable.len())
    };

    window.open_alert_dialog(cx, move |alert, _, cx| {
        let workspace = workspace.clone();
        let deletable = deletable.clone();
        alert
            .title(label.clone())
            .description(description.clone())
            .icon(Icon::new(IconName::TriangleAlert).text_color(cx.theme().danger))
            .confirm()
            .ok_text("Delete")
            .ok_variant(ButtonVariant::Danger)
            .cancel_text("Cancel")
            .on_ok(move |_, _, cx| {
                workspace.update(cx, |ws, cx| {
                    ws.delete_snapshots(deletable.clone(), cx);
                });
                true
            })
    });
}

pub fn open_rollback_confirm_dialog(
    workspace: Entity<WorkspaceModel>,
    snapshot: Snapshot,
    window: &mut Window,
    cx: &mut App,
) {
    let key = snapshot.key();
    let title = format!("Rollback to {} (#{})", key.config, key.number);
    let description = format!(
        "Snapshot: {}\nDescription: {}\n\nSnapper will create backup snapshot(s) and set the default Btrfs subvolume to snapshot #{}. A system reboot is required to boot into the rolled-back state.",
        key,
        if snapshot.description.is_empty() {
            "<no description>"
        } else {
            &snapshot.description
        },
        key.number
    );

    window.open_alert_dialog(cx, move |alert, _, cx| {
        let workspace = workspace.clone();
        let key = key.clone();
        alert
            .title(title.clone())
            .description(description.clone())
            .icon(Icon::new(IconName::TriangleAlert).text_color(cx.theme().danger))
            .confirm()
            .ok_text("Rollback")
            .ok_variant(ButtonVariant::Danger)
            .cancel_text("Cancel")
            .on_ok(move |_, _, cx| {
                workspace.update(cx, |ws, cx| {
                    ws.rollback_snapshot(key.clone(), cx);
                });
                true
            })
    });
}

pub fn open_undochange_confirm_dialog(
    workspace: Entity<WorkspaceModel>,
    config: String,
    from: u32,
    to: u32,
    files: Vec<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let preview: Vec<String> = files.iter().take(6).map(|p| format!("  • {p}")).collect();
    let more = if files.len() > 6 {
        format!("\n  …and {} more path(s)", files.len() - 6)
    } else {
        String::new()
    };
    let description = format!(
        "Run `snapper -c {config} undochange {from}..{to}` on {} selected path(s):\n{}{}\n\nThis overwrites live files in the target subvolume.",
        files.len(),
        preview.join("\n"),
        more
    );

    window.open_alert_dialog(cx, move |alert, _, cx| {
        let workspace = workspace.clone();
        let config = config.clone();
        let files = files.clone();
        alert
            .title("Restore Selected Files")
            .description(description.clone())
            .icon(Icon::new(IconName::TriangleAlert).text_color(cx.theme().warning))
            .confirm()
            .ok_text("Restore Files")
            .ok_variant(ButtonVariant::Danger)
            .cancel_text("Cancel")
            .on_ok(move |_, _, cx| {
                workspace.update(cx, |ws, cx| {
                    ws.undo_changes(config.clone(), from, to, files.clone(), cx);
                });
                true
            })
    });
}

pub fn open_cleanup_dialog(
    workspace: Entity<WorkspaceModel>,
    default_config: Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let configs = {
        let ws = workspace.read(cx);
        let mut names: Vec<ChoiceItem> = ws
            .configs()
            .iter()
            .map(|c| ChoiceItem::new(c.name.clone(), format!("{} ({})", c.name, c.subvolume)))
            .collect();
        if names.is_empty() {
            names.push(ChoiceItem::new("root", "root (/)"));
        }
        names
    };
    let selected_cfg_ix = default_config
        .as_deref()
        .and_then(|target| configs.iter().position(|c| c.value == target))
        .unwrap_or(0);

    let algorithms = vec![
        ChoiceItem::new("number", "number — Prune excess snapshots by NUMBER_* limits"),
        ChoiceItem::new("timeline", "timeline — Prune hourly/daily/monthly TIMELINE_* buckets"),
        ChoiceItem::new("empty-pre-post", "empty-pre-post — Remove empty pre/post snapshot pairs"),
    ];

    let config_select = cx.new(|cx| {
        SelectState::new(
            configs,
            Some(gpui_kit::component::IndexPath::new(selected_cfg_ix)),
            window,
            cx,
        )
    });
    let alg_select = cx.new(|cx| {
        SelectState::new(
            algorithms,
            Some(gpui_kit::component::IndexPath::new(0)),
            window,
            cx,
        )
    });
    let error_msg: Entity<Option<SharedString>> = cx.new(|_| None);
    let initial_focus = Rc::new(Cell::new(true));

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            config_select.focus_handle(cx).focus(window, cx);
        }

        let ws = workspace.clone();
        let cfg_sel = config_select.clone();
        let alg_sel = alg_select.clone();
        let err_state = error_msg.clone();
        let current_err = error_msg.read(cx).clone();

        dialog
            .title("Run Snapper Cleanup")
            .width(px(480.))
            .child(
                v_flex()
                    .gap_3()
                    .child(dialog_header_banner(
                        LucideIconName::Archive,
                        cx.theme().warning,
                        "Execute Snapper Retention Cleanup",
                        "Apply a Snapper cleanup algorithm immediately to prune eligible snapshots and reclaim space.",
                        cx,
                    ))
                    .child(
                        v_form()
                            .child(field().label("Configuration").child(Select::new(&config_select)))
                            .child(field().label("Algorithm").child(Select::new(&alg_select))),
                    )
                    .children(current_err.map(|err| {
                        div()
                            .text_sm()
                            .text_color(cx.theme().danger)
                            .child(err)
                    })),
            )
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cleanup-cancel")
                            .rounded(aether::pill_radius())
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                    )
                    .child(
                        Button::new("dialog-cleanup-submit")
                            .primary()
                            .rounded(aether::pill_radius())
                            .label("Run Cleanup")
                            .on_click(move |_, window, cx| {
                                let Some(cfg) = cfg_sel.read(cx).selected_value().cloned() else {
                                    err_state.update(cx, |e, cx| {
                                        *e = Some("Please select a configuration.".into());
                                        cx.notify();
                                    });
                                    return;
                                };
                                let alg_str = alg_sel
                                    .read(cx)
                                    .selected_value()
                                    .cloned()
                                    .unwrap_or_else(|| "number".to_string());
                                let alg = CleanupAlgorithm::from_cli_str(&alg_str)
                                    .unwrap_or(CleanupAlgorithm::Number);
                                ws.update(cx, |ws, cx| ws.run_cleanup(cfg, alg, cx));
                                window.close_dialog(cx);
                            }),
                    ),
            )
    });
}
