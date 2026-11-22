use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::table::{Column, ColumnSort, DataTable, TableDelegate, TableEvent, TableState};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex};
use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Stateful, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};
use snapper_core::format::format_size;
use snapper_core::model::{Snapshot, SnapshotKey};
use snapper_core::query::SortKey;

use crate::state::{InspectorTab, WorkspaceEvent, WorkspaceModel};
use crate::views::dialogs::compare::open_compare_dialog;
use crate::views::dialogs::confirm::{
    open_delete_confirm_dialog, open_rollback_confirm_dialog,
};
use crate::views::dialogs::modify::open_modify_snapshot_dialog;

pub struct SnapshotTableDelegate {
    workspace: Entity<WorkspaceModel>,
    columns: Vec<Column>,
    rows: Vec<Snapshot>,
    checked: std::collections::BTreeSet<SnapshotKey>,
    hydrating_space: bool,
    loading: bool,
}

impl SnapshotTableDelegate {
    pub fn new(workspace: Entity<WorkspaceModel>) -> Self {
        let columns = vec![
            Column::new("select", "")
                .width(px(38.))
                .resizable(false)
                .movable(false)
                .selectable(false),
            Column::new("number", "#").width(px(124.)).descending(),
            Column::new("config", "Config").width(px(96.)).sortable(),
            Column::new("type", "Type").width(px(108.)).sortable(),
            Column::new("date", "Date").width(px(156.)).sortable(),
            Column::new("space", "Space")
                .width(px(96.))
                .sortable()
                .text_right(),
            Column::new("cleanup", "Cleanup").width(px(100.)).sortable(),
            Column::new("description", "Description")
                .width(px(320.))
                .sortable(),
        ];

        Self {
            workspace,
            columns,
            rows: Vec::new(),
            checked: std::collections::BTreeSet::new(),
            hydrating_space: false,
            loading: false,
        }
    }

    pub fn sync_from_workspace(&mut self, cx: &App) {
        let ws = self.workspace.read(cx);
        self.rows = ws
            .filtered_indices()
            .iter()
            .filter_map(|&idx| ws.snapshots().get(idx).cloned())
            .collect();
        self.checked = ws.checked_keys().clone();
        self.hydrating_space = ws.hydrating_space();
        self.loading = ws.loading_snapshots() && self.rows.is_empty();

        let sort_col = match ws.sort_key() {
            SortKey::Number => 1,
            SortKey::Config => 2,
            SortKey::Type => 3,
            SortKey::Date => 4,
            SortKey::UsedSpace => 5,
            SortKey::Cleanup => 6,
            SortKey::Description => 7,
            SortKey::User => 1,
        };
        for (ix, col) in self.columns.iter_mut().enumerate() {
            if ix == 0 {
                continue;
            }
            col.sort = Some(if ix == sort_col {
                if ws.sort_asc() {
                    ColumnSort::Ascending
                } else {
                    ColumnSort::Descending
                }
            } else {
                ColumnSort::Default
            });
        }
    }

    fn space_display(&self, snap: &Snapshot) -> String {
        match snap.used_space {
            Some(bytes) => format_size(bytes),
            None if self.hydrating_space => "…".to_string(),
            None => "—".to_string(),
        }
    }
}

impl TableDelegate for SnapshotTableDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> Column {
        self.columns[col_ix].clone()
    }

    fn render_tr(
        &mut self,
        row_ix: usize,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        let id: SharedString = match self.rows.get(row_ix) {
            Some(row) => format!("snapshot-row-{}-{}", row.config, row.number).into(),
            None => format!("snapshot-row-{row_ix}").into(),
        };
        div().id(id)
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.get(row_ix).cloned() else {
            return div().into_any_element();
        };
        let key = row.key();
        let ws = self.workspace.clone();

        match col_ix {
            0 => {
                let checked = self.checked.contains(&key);
                let check_id: SharedString =
                    format!("table-check-{}-{}", key.config, key.number).into();
                h_flex()
                    .w_full()
                    .justify_center()
                    .items_center()
                    .child(Checkbox::new(check_id).checked(checked).on_click(move |_, _, cx| {
                        ws.update(cx, |ws, cx| ws.toggle_checked(key.clone(), cx));
                    }))
                    .into_any_element()
            }
            1 => {
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .font_family(cx.theme().mono_font_family.clone())
                            .font_semibold()
                            .text_color(if row.number == 0 {
                                cx.theme().info
                            } else {
                                cx.theme().foreground
                            })
                            .child(format!("#{}", row.number)),
                    )
                    .when(row.active, |this| {
                        this.child(Tag::success().xsmall().outline().child("act"))
                    })
                    .when(row.default, |this| {
                        this.child(Tag::info().xsmall().outline().child("def"))
                    })
                    .when(row.is_pinned(), |this| {
                        this.child(
                            Icon::new(IconName::StarFill)
                                .xsmall()
                                .text_color(cx.theme().warning),
                        )
                    })
                    .when(!row.is_pinned() && row.is_important(), |this| {
                        this.child(
                            Icon::new(IconName::Asterisk)
                                .xsmall()
                                .text_color(cx.theme().accent_foreground),
                        )
                    })
                    .into_any_element()
            }
            2 => div()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(row.config.clone())
                .into_any_element(),
            3 => {
                let type_label = match row.snapshot_type.as_str() {
                    "pre" => "pre".to_string(),
                    "post" => "post".to_string(),
                    _ => {
                        if row.number == 0 {
                            "current".to_string()
                        } else {
                            "single".to_string()
                        }
                    }
                };
                let tag = match row.snapshot_type.as_str() {
                    "pre" => Tag::warning().xsmall().outline().child(type_label),
                    "post" => Tag::info().xsmall().outline().child(type_label),
                    _ => Tag::secondary().xsmall().child(type_label),
                };
                let pair_num = if row.is_pre() {
                    row.post_number
                } else if row.is_post() {
                    row.pre_number
                } else {
                    None
                };
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(tag)
                    .when_some(pair_num, |this, num| {
                        let cfg = row.config.clone();
                        let ws_jump = ws.clone();
                        let jump_id: SharedString =
                            format!("pair-jump-{}-{}-{}", row.config, row.number, num).into();
                        this.child(
                            Button::new(jump_id)
                                .xsmall()
                                .ghost()
                                .label(format!("→#{num}"))
                                .tooltip(format!("Select paired snapshot #{num}"))
                                .on_click(move |_, _, cx| {
                                    ws_jump.update(cx, |ws, cx| {
                                        ws.select_key(SnapshotKey::new(cfg.clone(), num), cx);
                                    });
                                }),
                        )
                    })
                    .into_any_element()
            }
            4 => div()
                .text_xs()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().muted_foreground)
                .child(row.date.clone())
                .into_any_element(),
            5 => div()
                .w_full()
                .text_right()
                .text_xs()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(if row.used_space.is_some() {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                })
                .child(self.space_display(&row))
                .into_any_element(),
            6 => {
                let cleanup = row.cleanup.clone().unwrap_or_else(|| "—".to_string());
                div()
                    .text_xs()
                    .text_color(if row.cleanup.is_some() {
                        cx.theme().foreground
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(cleanup)
                    .into_any_element()
            }
            7 => {
                let desc = if row.description.is_empty() {
                    if row.number == 0 {
                        "Live filesystem state".to_string()
                    } else {
                        "—".to_string()
                    }
                } else {
                    row.description.clone()
                };
                div()
                    .text_sm()
                    .text_color(if row.description.is_empty() {
                        cx.theme().muted_foreground
                    } else {
                        cx.theme().foreground
                    })
                    .truncate()
                    .child(desc)
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _cx: &App) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        let label = match col_ix {
            0 => if self.checked.contains(&row.key()) {
                "Selected"
            } else {
                "Not selected"
            }
            .to_string(),
            1 => format!("#{}", row.number),
            2 => row.config.clone(),
            3 => row.snapshot_type.clone(),
            4 => row.date.clone(),
            5 => self.space_display(row),
            6 => row.cleanup.clone().unwrap_or_else(|| "none".to_string()),
            7 => row.description.clone(),
            _ => return None,
        };
        Some(label.into())
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        let sort_key = match col_ix {
            1 => SortKey::Number,
            2 => SortKey::Config,
            3 => SortKey::Type,
            4 => SortKey::Date,
            5 => SortKey::UsedSpace,
            6 => SortKey::Cleanup,
            7 => SortKey::Description,
            _ => return,
        };
        let asc = matches!(sort, ColumnSort::Ascending);
        for (ix, col) in self.columns.iter_mut().enumerate() {
            if ix == 0 {
                continue;
            }
            col.sort = Some(if ix == col_ix {
                sort
            } else {
                ColumnSort::Default
            });
        }
        let ws = self.workspace.clone();
        // Defer workspace update so TableState drops its delegate borrow before event subscribers refresh the table.
        cx.spawn(async move |_, cx| {
            cx.update(|cx| {
                ws.update(cx, |ws, cx| ws.set_sort(sort_key, asc, cx));
            });
        })
        .detach();
    }

    fn loading(&self, _cx: &App) -> bool {
        self.loading
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let err = self.workspace.read(cx).last_error().map(ToOwned::to_owned);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .p_6()
            .child(
                Icon::new(IconName::Inbox)
                    .size_10()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child(if err.is_some() {
                        "Unable to load snapshots"
                    } else {
                        "No snapshots match your filters"
                    }),
            )
            .when_some(err, |this, msg| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(msg),
                )
            })
            .into_any_element()
    }

    fn context_menu(
        &mut self,
        row_ix: usize,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut Context<TableState<Self>>,
    ) -> PopupMenu {
        let Some(snap) = self.rows.get(row_ix).cloned() else {
            return menu;
        };
        let key = snap.key();
        let ws_changes = self.workspace.clone();
        let ws_compare = self.workspace.clone();
        let ws_pin = self.workspace.clone();
        let ws_mod = self.workspace.clone();
        let ws_rb = self.workspace.clone();
        let ws_del = self.workspace.clone();

        let key_changes = key.clone();
        let key_pin = key.clone();
        let snap_compare = snap.clone();
        let snap_mod = snap.clone();
        let snap_rb = snap.clone();
        let snap_del = snap.clone();

        let is_pinned = snap.is_pinned();
        let is_protected = snap.is_protected();
        let is_zero = snap.number == 0;

        menu.item(
            PopupMenuItem::new("Inspect Changes")
                .icon(IconName::FileText)
                .on_click(move |_, _, cx| {
                    ws_changes.update(cx, |ws, cx| {
                        ws.select_key(key_changes.clone(), cx);
                        ws.set_inspector_tab(InspectorTab::Changes, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Compare With…")
                .icon(IconName::Replace)
                .on_click(move |_, window, cx| {
                    open_compare_dialog(
                        ws_compare.clone(),
                        Some((snap_compare.config.clone(), snap_compare.number, 0)),
                        window,
                        cx,
                    );
                }),
        )
        .item(PopupMenuItem::separator())
        .item(
            PopupMenuItem::new(if is_pinned {
                "Unpin Snapshot"
            } else {
                "Pin Snapshot"
            })
            .icon(if is_pinned {
                IconName::StarOff
            } else {
                IconName::Star
            })
            .on_click(move |_, _, cx| {
                ws_pin.update(cx, |ws, cx| {
                    ws.select_key(key_pin.clone(), cx);
                    ws.toggle_pin_selected(cx);
                });
            }),
        )
        .item(
            PopupMenuItem::new("Modify Snapshot…")
                .icon(IconName::Settings2)
                .on_click(move |_, window, cx| {
                    open_modify_snapshot_dialog(ws_mod.clone(), snap_mod.clone(), window, cx);
                }),
        )
        .item(PopupMenuItem::separator())
        .item(
            PopupMenuItem::new("Rollback…")
                .icon(IconName::Undo2)
                .disabled(is_zero)
                .on_click(move |_, window, cx| {
                    open_rollback_confirm_dialog(ws_rb.clone(), snap_rb.clone(), window, cx);
                }),
        )
        .item(
            PopupMenuItem::new("Delete…")
                .icon(IconName::Delete)
                .disabled(is_protected)
                .on_click(move |_, window, cx| {
                    open_delete_confirm_dialog(ws_del.clone(), vec![snap_del.clone()], window, cx);
                }),
        )
    }
}

pub struct SnapshotTableView {
    workspace: Entity<WorkspaceModel>,
    table: Entity<TableState<SnapshotTableDelegate>>,
    _subs: Vec<Subscription>,
}

impl SnapshotTableView {
    pub fn new(
        workspace: Entity<WorkspaceModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut delegate = SnapshotTableDelegate::new(workspace.clone());
        delegate.sync_from_workspace(cx);

        let table = cx.new(|cx| {
            TableState::new(delegate, window, cx)
                .row_selectable(true)
                .sortable(true)
                .col_resizable(true)
        });

        let table_sub = cx.subscribe_in(&table, window, |this, _, event: &TableEvent, _, cx| {
            if let TableEvent::SelectRow(row_ix) = event {
                let row_ix = *row_ix;
                this.workspace
                    .update(cx, |ws, cx| ws.select_filtered_row(row_ix, cx));
            } else if let TableEvent::DoubleClickedRow(row_ix) = event {
                let row_ix = *row_ix;
                this.workspace.update(cx, |ws, cx| {
                    ws.select_filtered_row(row_ix, cx);
                    ws.set_inspector_tab(InspectorTab::Changes, cx);
                });
            }
        });

        let ws_sub = cx.subscribe(&workspace, |this, ws, event: &WorkspaceEvent, cx| {
            match event {
                WorkspaceEvent::SnapshotsChanged => {
                    let selected_row = ws.read(cx).selected_filtered_row();
                    this.table.update(cx, |table, cx| {
                        table.delegate_mut().sync_from_workspace(cx);
                        table.refresh(cx);
                        if let Some(row_ix) = selected_row {
                            table.set_selected_row(row_ix, cx);
                        }
                    });
                    cx.notify();
                }
                WorkspaceEvent::SelectionChanged => {
                    let selected_row = ws.read(cx).selected_filtered_row();
                    this.table.update(cx, |table, cx| {
                        if let Some(row_ix) = selected_row
                            && table.selected_row() != Some(row_ix)
                        {
                            table.set_selected_row(row_ix, cx);
                        }
                    });
                    cx.notify();
                }
                _ => {}
            }
        });

        Self {
            workspace,
            table,
            _subs: vec![table_sub, ws_sub],
        }
    }

    pub fn table_state(&self) -> &Entity<TableState<SnapshotTableDelegate>> {
        &self.table
    }
}

impl Render for SnapshotTableView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (show_empty_panel, auth_required, supports_auth, last_error) = {
            let ws = self.workspace.read(cx);
            let show = ws.snapshots().is_empty()
                && !ws.loading_snapshots()
                && (ws.auth_required() || ws.last_error().is_some());
            (
                show,
                ws.auth_required(),
                ws.supports_password_auth(),
                ws.last_error().map(ToOwned::to_owned),
            )
        };

        div()
            .id("snapshot-table-container")
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().table)
            .child(if show_empty_panel {
                let ws_auth = self.workspace.clone();
                let ws_retry = self.workspace.clone();
                let heading = if auth_required {
                    "Administrator Authentication Required"
                } else {
                    "Failed to Load Snapshots"
                };
                let icon_name = if auth_required {
                    LucideIconName::Lock
                } else {
                    LucideIconName::TriangleAlert
                };

                gpui_kit::component::v_flex()
                    .id("snapshot-table-auth-state")
                    .test_support()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(
                        gpui_kit::component::v_flex()
                            .max_w(px(460.))
                            .gap_3()
                            .p_6()
                            .rounded_lg()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .items_center()
                            .child(
                                Icon::new(icon_name)
                                    .size_10()
                                    .text_color(if auth_required {
                                        cx.theme().warning
                                    } else {
                                        cx.theme().danger
                                    }),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child(heading),
                            )
                            .when_some(last_error, |this, err| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .text_center()
                                        .child(err),
                                )
                            })
                            .child(
                                h_flex()
                                    .gap_2()
                                    .pt_2()
                                    .when(auth_required || supports_auth, |this| {
                                        this.child(
                                            Button::new("table-auth-btn")
                                                .primary()
                                                .icon(Icon::new(LucideIconName::Lock))
                                                .label("Authenticate…")
                                                .on_click(move |_, _, cx| {
                                                    ws_auth.update(cx, |ws, cx| {
                                                        ws.request_authentication(cx);
                                                    });
                                                }),
                                        )
                                    })
                                    .child(
                                        Button::new("table-retry-btn")
                                            .icon(IconName::RefreshCw)
                                            .label("Retry")
                                            .on_click(move |_, _, cx| {
                                                ws_retry.update(cx, |ws, cx| ws.refresh(cx));
                                            }),
                                    ),
                            ),
                    )
                    .into_any_element()
            } else {
                DataTable::new(&self.table)
                    .stripe(true)
                    .bordered(false)
                    .small()
                    .into_any_element()
            })
    }
}
