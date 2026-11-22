use std::collections::{BTreeSet, HashMap};

use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::diff::{Diff, DiffFile, DiffMode, DiffParseOptions, DiffState};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::resizable::{resizable_panel, v_resizable};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::tree::{TreeItem, TreeState, tree};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use snapper_core::model::{ContentChange, FileTreeNode};

use crate::state::{WorkspaceEvent, WorkspaceModel};
use crate::theme::ThemeAccents;
use crate::views::aether;
use crate::views::dialogs::compare::open_compare_dialog;
use crate::views::dialogs::confirm::open_undochange_confirm_dialog;

#[derive(Clone, Debug)]
struct NodeMeta {
    full_path: String,
    change: ContentChange,
    is_leaf: bool,
    leaf_count: usize,
}

fn change_badge(change: ContentChange) -> gpui_kit::AnyElement {
    match change {
        ContentChange::Created => Tag::success()
            .xsmall()
            .rounded_full()
            .border_0()
            .child("A")
            .into_any_element(),
        ContentChange::Modified => Tag::warning()
            .xsmall()
            .rounded_full()
            .border_0()
            .child("M")
            .into_any_element(),
        ContentChange::Deleted => Tag::danger()
            .xsmall()
            .rounded_full()
            .border_0()
            .child("D")
            .into_any_element(),
        ContentChange::Unchanged => Tag::info()
            .xsmall()
            .rounded_full()
            .border_0()
            .child("Attr")
            .into_any_element(),
    }
}

fn collect_leaf_paths_filtered(
    nodes: &[FileTreeNode],
    target_dir: &str,
    query_lower: &str,
    out: &mut Vec<String>,
) -> bool {
    for node in nodes {
        if node.full_path == target_dir {
            collect_descendant_leaves(node, query_lower, out);
            return true;
        }
        if !node.children.is_empty()
            && target_dir.starts_with(node.full_path.as_str())
            && collect_leaf_paths_filtered(&node.children, target_dir, query_lower, out)
        {
            return true;
        }
    }
    false
}

fn collect_descendant_leaves(node: &FileTreeNode, query_lower: &str, out: &mut Vec<String>) {
    if node.children.is_empty() {
        if query_lower.is_empty() || node.full_path.to_lowercase().contains(query_lower) {
            out.push(node.full_path.clone());
        }
    } else {
        for child in &node.children {
            collect_descendant_leaves(child, query_lower, out);
        }
    }
}

fn build_filtered_tree_items(
    nodes: &[FileTreeNode],
    query_lower: &str,
    auto_expand: bool,
    meta: &mut HashMap<SharedString, NodeMeta>,
    visible_leaves: &mut Vec<String>,
) -> (Vec<TreeItem>, usize) {
    let mut items = Vec::new();
    let mut subtree_leaves = 0usize;
    for node in nodes {
        let is_leaf = node.children.is_empty();
        if is_leaf {
            if !query_lower.is_empty() && !node.full_path.to_lowercase().contains(query_lower) {
                continue;
            }
            let change = node
                .change
                .as_ref()
                .map(|c| c.flags.content)
                .unwrap_or(ContentChange::Unchanged);
            let id: SharedString = node.id.clone().into();
            visible_leaves.push(node.full_path.clone());
            meta.insert(
                id.clone(),
                NodeMeta {
                    full_path: node.full_path.clone(),
                    change,
                    is_leaf: true,
                    leaf_count: 1,
                },
            );
            items.push(TreeItem::new(id, node.label.clone()));
            subtree_leaves += 1;
        } else {
            let (children, child_leaves) = build_filtered_tree_items(
                &node.children,
                query_lower,
                auto_expand,
                meta,
                visible_leaves,
            );
            if children.is_empty() {
                continue;
            }
            let change = node
                .change
                .as_ref()
                .map(|c| c.flags.content)
                .unwrap_or(ContentChange::Unchanged);
            let id: SharedString = node.id.clone().into();
            meta.insert(
                id.clone(),
                NodeMeta {
                    full_path: node.full_path.clone(),
                    change,
                    is_leaf: false,
                    leaf_count: child_leaves,
                },
            );
            subtree_leaves += child_leaves;
            items.push(
                TreeItem::new(id, node.label.clone())
                    .expanded(auto_expand)
                    .children(children),
            );
        }
    }
    (items, subtree_leaves)
}

pub struct DiffPaneView {
    workspace: Entity<WorkspaceModel>,
    filter_input: Entity<InputState>,
    tree_state: Entity<TreeState>,
    diff_state: Entity<DiffState>,
    node_meta: HashMap<SharedString, NodeMeta>,
    visible_leaves: Vec<String>,
    raw_fallback: Option<SharedString>,
    parse_error: Option<SharedString>,
    _subs: Vec<Subscription>,
}

impl DiffPaneView {
    pub fn new(
        workspace: Entity<WorkspaceModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter changed file paths…"));
        let tree_state = cx.new(|cx| TreeState::new(cx));
        let diff_state = cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Unified));

        let filter_sub = cx.subscribe_in(
            &filter_input,
            window,
            |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.rebuild_tree(cx);
                    cx.notify();
                }
            },
        );

        let ws_sub = cx.subscribe(&workspace, |this, _, event: &WorkspaceEvent, cx| {
            match event {
                WorkspaceEvent::StatusChanged => {
                    this.rebuild_tree(cx);
                    cx.notify();
                }
                WorkspaceEvent::DiffChanged => {
                    this.sync_diff_from_workspace(cx);
                    cx.notify();
                }
                _ => {}
            }
        });

        let mut view = Self {
            workspace,
            filter_input,
            tree_state,
            diff_state,
            node_meta: HashMap::new(),
            visible_leaves: Vec::new(),
            raw_fallback: None,
            parse_error: None,
            _subs: vec![filter_sub, ws_sub],
        };
        view.rebuild_tree(cx);
        view.sync_diff_from_workspace(cx);
        view
    }

    pub fn set_diff_mode(&mut self, mode: DiffMode, cx: &mut Context<Self>) {
        self.diff_state.update(cx, |state, cx| state.set_mode(mode, cx));
        cx.notify();
    }

    pub fn toggle_diff_mode(&mut self, cx: &mut Context<Self>) -> DiffMode {
        let next = match self.diff_state.read(cx).mode() {
            DiffMode::Unified => DiffMode::Split,
            DiffMode::Split | _ => DiffMode::Unified,
        };
        self.set_diff_mode(next, cx);
        next
    }

    pub fn next_change(&mut self, cx: &mut Context<Self>) {
        self.diff_state.update(cx, |state, cx| state.next_change(cx));
    }

    pub fn previous_change(&mut self, cx: &mut Context<Self>) {
        self.diff_state
            .update(cx, |state, cx| state.previous_change(cx));
    }

    fn rebuild_tree(&mut self, cx: &mut Context<Self>) {
        let (items, meta, visible_leaves, selected_file) = {
            let query_lower = self.filter_input.read(cx).value().trim().to_lowercase();
            let ws = self.workspace.read(cx);
            let auto_expand = ws.file_changes().len() <= 250 || !query_lower.is_empty();
            let selected_file = ws.selected_file().map(ToOwned::to_owned);
            let mut meta = HashMap::new();
            let mut visible_leaves = Vec::new();
            let (items, _) = build_filtered_tree_items(
                ws.file_tree(),
                &query_lower,
                auto_expand,
                &mut meta,
                &mut visible_leaves,
            );
            (items, meta, visible_leaves, selected_file)
        };
        self.node_meta = meta;
        self.visible_leaves = visible_leaves;

        self.tree_state.update(cx, |state, cx| {
            state.set_items(items, cx);
            if let Some(target) = &selected_file
                && let Some((id, _)) = self
                    .node_meta
                    .iter()
                    .find(|(_, m)| m.is_leaf && m.full_path == *target)
                && let Some(ix) = state.index_of(id)
            {
                state.set_selected_index(Some(ix), cx);
            }
        });
    }

    fn sync_diff_from_workspace(&mut self, cx: &mut Context<Self>) {
        let (is_binary, output) = {
            let ws = self.workspace.read(cx);
            (ws.diff_is_binary(), ws.diff_output().cloned())
        };

        self.raw_fallback = None;
        self.parse_error = None;

        if is_binary {
            self.diff_state
                .update(cx, |state, cx| state.set_files([], cx));
            return;
        }

        let Some(output) = output else {
            self.diff_state
                .update(cx, |state, cx| state.set_files([], cx));
            return;
        };

        if output.patch.trim().is_empty() {
            self.raw_fallback = Some(
                "No textual diff output (file may only have permission, ownership, or metadata changes)."
                    .into(),
            );
            self.diff_state
                .update(cx, |state, cx| state.set_files([], cx));
            return;
        }

        let opts = DiffParseOptions::new().truncated_hunks(true);
        match DiffFile::parse_with(&output.patch, opts) {
            Ok(files) if !files.is_empty() => {
                self.diff_state
                    .update(cx, |state, cx| state.set_files(files, cx));
            }
            Ok(_) => {
                self.raw_fallback = Some(output.patch.into());
                self.diff_state
                    .update(cx, |state, cx| state.set_files([], cx));
            }
            Err(err) => {
                self.parse_error = Some(err.to_string().into());
                self.raw_fallback = Some(output.patch.into());
                self.diff_state
                    .update(cx, |state, cx| state.set_files([], cx));
            }
        }
    }
}

impl Render for DiffPaneView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let compare_range = ws
            .compare_range()
            .map(|(cfg, f, t)| (cfg.to_string(), f, t));
        let compare_pinned = ws.compare_pinned();
        let status_loading = ws.status_loading();
        let status_error = ws.status_error().map(ToOwned::to_owned);
        let selected_file = ws.selected_file().map(ToOwned::to_owned);
        let selected_change = selected_file.as_deref().and_then(|path| {
            ws.file_changes()
                .iter()
                .find(|c| c.path == path)
                .map(|c| c.flags.content)
        });
        let checked_files: BTreeSet<String> = ws.checked_files().clone();
        let total_changes = ws.file_changes().len();
        let (mut added_cnt, mut modified_cnt, mut deleted_cnt, mut attr_cnt) = (0usize, 0, 0, 0);
        for fc in ws.file_changes() {
            match fc.flags.content {
                ContentChange::Created => added_cnt += 1,
                ContentChange::Modified => modified_cnt += 1,
                ContentChange::Deleted => deleted_cnt += 1,
                ContentChange::Unchanged => attr_cnt += 1,
            }
        }

        let diff_loading = ws.diff_loading();
        let diff_error = ws.diff_error().map(ToOwned::to_owned);
        let diff_is_binary = ws.diff_is_binary();
        let diff_truncated = ws.diff_output().is_some_and(|d| d.truncated);
        let diff_total_bytes = ws.diff_output().map(|d| d.total_bytes).unwrap_or(0);

        let diff_mode = self.diff_state.read(cx).mode();
        let view_entity = cx.entity().clone();
        let visible_leaves = self.visible_leaves.clone();
        let checked_count = checked_files.len();

        let ws_cmp = self.workspace.clone();
        let ws_reset = self.workspace.clone();
        let ws_all = self.workspace.clone();
        let ws_none = self.workspace.clone();
        let ws_restore = self.workspace.clone();

        let range_for_cmp = compare_range.clone();
        let range_for_restore = compare_range.clone();
        let checked_list_for_restore: Vec<String> = checked_files.iter().cloned().collect();

        let node_meta_for_tree = self.node_meta.clone();
        let checked_for_tree = checked_files.clone();
        let selected_for_tree = selected_file.clone();
        let ws_for_tree = self.workspace.clone();
        let filter_input_for_tree = self.filter_input.clone();
        let ws_for_ctx = self.workspace.clone();
        let node_meta_for_ctx = self.node_meta.clone();
        let range_for_ctx = compare_range.clone();

        let accents = ThemeAccents::from_theme(cx.theme());

        v_flex()
            .id("diff-pane")
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(
                // Top comparison controls + blast-radius summary
                h_flex()
                    .w_full()
                    .px_3p5()
                    .py_2()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .bg(accents.header_tint)
                    .child(
                        h_flex()
                            .gap_1p5()
                            .flex_wrap()
                            .items_center()
                            .child(match &compare_range {
                                Some((cfg, from, to)) => Tag::info()
                                    .small()
                                    .rounded_full()
                                    .border_0()
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .items_center()
                                            .child(Icon::new(LucideIconName::GitCompareArrows).xsmall())
                                            .child(format!("{cfg}: #{from} .. #{to}")),
                                    )
                                    .into_any_element(),
                                None => Tag::secondary()
                                    .small()
                                    .rounded_full()
                                    .border_0()
                                    .child("No compare range")
                                    .into_any_element(),
                            })
                            .when(added_cnt > 0, |this| {
                                this.child(
                                    Tag::success()
                                        .xsmall()
                                        .rounded_full()
                                        .border_0()
                                        .child(format!("+{added_cnt}")),
                                )
                            })
                            .when(modified_cnt > 0, |this| {
                                this.child(
                                    Tag::warning()
                                        .xsmall()
                                        .rounded_full()
                                        .border_0()
                                        .child(format!("~{modified_cnt}")),
                                )
                            })
                            .when(deleted_cnt > 0, |this| {
                                this.child(
                                    Tag::danger()
                                        .xsmall()
                                        .rounded_full()
                                        .border_0()
                                        .child(format!("-{deleted_cnt}")),
                                )
                            })
                            .when(attr_cnt > 0, |this| {
                                this.child(
                                    Tag::info()
                                        .xsmall()
                                        .rounded_full()
                                        .border_0()
                                        .child(format!("Attr {attr_cnt}")),
                                )
                            })
                            .child(
                                Button::new("diff-change-compare")
                                    .xsmall()
                                    .secondary()
                                    .rounded(aether::pill_radius())
                                    .icon(Icon::new(LucideIconName::GitCompareArrows))
                                    .label("Compare…")
                                    .on_click(move |_, window, cx| {
                                        open_compare_dialog(
                                            ws_cmp.clone(),
                                            range_for_cmp.clone(),
                                            window,
                                            cx,
                                        );
                                    }),
                            )
                            .when(compare_pinned, |this| {
                                this.child(
                                    Button::new("diff-reset-compare")
                                        .xsmall()
                                        .ghost()
                                        .rounded(aether::pill_radius())
                                        .label("Follow Selection")
                                        .on_click(move |_, _, cx| {
                                            ws_reset.update(cx, |ws, cx| ws.clear_custom_compare(cx));
                                        }),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .child(
                                h_flex()
                                    .p(px(2.))
                                    .gap(px(2.))
                                    .items_center()
                                    .rounded_full()
                                    .bg(accents.sunken_bg)
                                    .child({
                                        let btn = Button::new("diff-mode-unified")
                                            .xsmall()
                                            .rounded(aether::pill_radius())
                                            .label("Unified")
                                            .on_click({
                                                let view = view_entity.clone();
                                                move |_, _, cx| {
                                                    view.update(cx, |v, cx| {
                                                        v.set_diff_mode(DiffMode::Unified, cx);
                                                    });
                                                }
                                            });
                                        if diff_mode == DiffMode::Unified {
                                            btn.primary()
                                        } else {
                                            btn.ghost()
                                        }
                                    })
                                    .child({
                                        let btn = Button::new("diff-mode-split")
                                            .xsmall()
                                            .rounded(aether::pill_radius())
                                            .label("Split")
                                            .on_click({
                                                let view = view_entity.clone();
                                                move |_, _, cx| {
                                                    view.update(cx, |v, cx| {
                                                        v.set_diff_mode(DiffMode::Split, cx);
                                                    });
                                                }
                                            });
                                        if diff_mode == DiffMode::Split {
                                            btn.primary()
                                        } else {
                                            btn.ghost()
                                        }
                                    }),
                            )
                            .child(
                                Button::new("diff-prev-change")
                                    .xsmall()
                                    .ghost()
                                    .rounded(aether::pill_radius())
                                    .icon(IconName::ArrowUp)
                                    .tooltip("Previous change hunk")
                                    .on_click({
                                        let view = view_entity.clone();
                                        move |_, _, cx| {
                                            view.update(cx, |v, cx| v.previous_change(cx));
                                        }
                                    }),
                            )
                            .child(
                                Button::new("diff-next-change")
                                    .xsmall()
                                    .ghost()
                                    .rounded(aether::pill_radius())
                                    .icon(IconName::ArrowDown)
                                    .tooltip("Next change hunk")
                                    .on_click({
                                        let view = view_entity.clone();
                                        move |_, _, cx| {
                                            view.update(cx, |v, cx| v.next_change(cx));
                                        }
                                    }),
                            )
                            .child(
                                Button::new("diff-restore-selected")
                                    .xsmall()
                                    .danger()
                                    .rounded(aether::pill_radius())
                                    .icon(IconName::Undo2)
                                    .label(if checked_count > 0 {
                                        format!("Restore {checked_count}…")
                                    } else {
                                        "Restore…".to_string()
                                    })
                                    .disabled(checked_count == 0 || range_for_restore.is_none())
                                    .on_click(move |_, window, cx| {
                                        if let Some((cfg, from, to)) = &range_for_restore {
                                            open_undochange_confirm_dialog(
                                                ws_restore.clone(),
                                                cfg.clone(),
                                                *from,
                                                *to,
                                                checked_list_for_restore.clone(),
                                                window,
                                                cx,
                                            );
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                div().flex_1().min_h_0().min_w_0().w_full().child(
                    v_resizable("diff-pane-split")
                        .with_handle_appearance(aether::soft_resize_handle())
                        .child(
                            resizable_panel()
                                .size(px(210.))
                                .size_range(px(110.)..px(420.))
                                .child(
                                    v_flex()
                                        .size_full()
                                        .min_h_0()
                                        .min_w_0()
                                        .child(
                                            h_flex()
                                                .w_full()
                                                .px_3()
                                                .py_1p5()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div().flex_1().min_w_0().child(
                                                        Input::new(&self.filter_input)
                                                            .id("diff-file-filter")
                                                            .xsmall()
                                                            .cleanable(true)
                                                            .prefix(
                                                                Icon::new(IconName::Search)
                                                                    .xsmall()
                                                                    .text_color(
                                                                        cx.theme().muted_foreground,
                                                                    ),
                                                            ),
                                                    ),
                                                )
                                                .child(
                                                    Button::new("diff-check-all")
                                                        .xsmall()
                                                        .ghost()
                                                        .rounded(aether::pill_radius())
                                                        .label("All")
                                                        .disabled(visible_leaves.is_empty())
                                                        .on_click({
                                                            let leaves = visible_leaves.clone();
                                                            move |_, _, cx| {
                                                                ws_all.update(cx, |ws, cx| {
                                                                    ws.set_checked_files_for_paths(
                                                                        leaves.clone(),
                                                                        true,
                                                                        cx,
                                                                    );
                                                                });
                                                            }
                                                        }),
                                                )
                                                .child(
                                                    Button::new("diff-check-none")
                                                        .xsmall()
                                                        .ghost()
                                                        .rounded(aether::pill_radius())
                                                        .label("None")
                                                        .disabled(checked_count == 0)
                                                        .on_click(move |_, _, cx| {
                                                            let paths: Vec<String> = ws_none
                                                                .read(cx)
                                                                .checked_files()
                                                                .iter()
                                                                .cloned()
                                                                .collect();
                                                            ws_none.update(cx, |ws, cx| {
                                                                ws.set_checked_files_for_paths(
                                                                    paths, false, cx,
                                                                );
                                                            });
                                                        }),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("diff-file-tree-host")
                                                .flex_1()
                                                .min_h_0()
                                                .min_w_0()
                                                .w_full()
                                                .map(|host| {
                                                    if status_loading {
                                                        host.child(aether::skeleton_rows(6))
                                                            .into_any_element()
                                                    } else if let Some(err) = status_error {
                                                        host.p_3()
                                                            .child(Alert::error(
                                                                "diff-status-error",
                                                                err,
                                                            ))
                                                            .into_any_element()
                                                    } else if total_changes == 0 {
                                                        host.child(aether::empty_state_card(
                                                            "diff-tree-empty",
                                                            LucideIconName::FolderTree,
                                                            cx.theme().muted_foreground,
                                                            "No changed files",
                                                            "No file deltas were recorded across this snapshot range.",
                                                            None,
                                                            cx,
                                                        ))
                                                        .into_any_element()
                                                    } else if self.visible_leaves.is_empty() {
                                                        host.child(aether::empty_state_card(
                                                            "diff-tree-filter-empty",
                                                            LucideIconName::Search,
                                                            cx.theme().info,
                                                            "No matching file paths",
                                                            "Adjust the path filter above to view changed files.",
                                                            None,
                                                            cx,
                                                        ))
                                                        .into_any_element()
                                                    } else {
                                                        host.child(
                                                            tree(&self.tree_state, move |ix, entry, _selected, _window, cx| {
                                                                let item_id = entry.item().id.clone();
                                                                let meta = node_meta_for_tree.get(&item_id).cloned();
                                                                let is_folder = entry.is_folder();
                                                                let depth = entry.depth();

                                                                let (is_checked, is_active_file, badge_el) =
                                                                    if let Some(m) = &meta {
                                                                        let checked = if m.is_leaf {
                                                                            checked_for_tree.contains(&m.full_path)
                                                                        } else {
                                                                            let prefix = format!(
                                                                                "{}/",
                                                                                m.full_path.trim_end_matches('/')
                                                                            );
                                                                            checked_for_tree
                                                                                .iter()
                                                                                .any(|p| p == &m.full_path || p.starts_with(&prefix))
                                                                        };
                                                                        let active = m.is_leaf
                                                                            && selected_for_tree.as_deref() == Some(m.full_path.as_str());
                                                                        let badge = if m.is_leaf {
                                                                            change_badge(m.change)
                                                                        } else {
                                                                            div()
                                                                                .text_xs()
                                                                                .font_family(cx.theme().mono_font_family.clone())
                                                                                .text_color(cx.theme().muted_foreground)
                                                                                .child(format!("{}", m.leaf_count))
                                                                                .into_any_element()
                                                                        };
                                                                        (checked, active, badge)
                                                                    } else {
                                                                        (false, false, div().into_any_element())
                                                                    };

                                                                let ws_click = ws_for_tree.clone();
                                                                let ws_chk = ws_for_tree.clone();
                                                                let meta_click = meta.clone();
                                                                let meta_chk = meta.clone();

                                                                let icon_name = if is_folder {
                                                                    if entry.is_expanded() {
                                                                        IconName::FolderOpen
                                                                    } else {
                                                                        IconName::Folder
                                                                    }
                                                                } else {
                                                                    IconName::FileText
                                                                };

                                                                let tree_item_id: SharedString = format!("file-tree-row-{ix}").into();
                                                                let chk_id: SharedString = format!("file-tree-check-{ix}").into();

                                                                ListItem::new(tree_item_id)
                                                                    .pl(px(8. + depth as f32 * 14.))
                                                                    .selected(is_active_file)
                                                                    .on_click(move |_, _, cx| {
                                                                        if let Some(m) = &meta_click
                                                                            && m.is_leaf
                                                                        {
                                                                            let path = m.full_path.clone();
                                                                            ws_click.update(cx, |ws, cx| {
                                                                                ws.select_file(path, cx);
                                                                            });
                                                                        }
                                                                    })
                                                                    .child(
                                                                        h_flex()
                                                                            .w_full()
                                                                            .gap_2()
                                                                            .items_center()
                                                                            .justify_between()
                                                                            .child(
                                                                                h_flex()
                                                                                    .flex_1()
                                                                                    .min_w_0()
                                                                                    .gap_2()
                                                                                    .items_center()
                                                                                    .child(
                                                                                        Checkbox::new(chk_id)
                                                                                            .checked(is_checked)
                                                                                            .on_click({
                                                                                                let filter_in = filter_input_for_tree.clone();
                                                                                                move |val, _, cx| {
                                                                                                    if let Some(m) = &meta_chk {
                                                                                                        if m.is_leaf {
                                                                                                            let path = m.full_path.clone();
                                                                                                            ws_chk.update(cx, |ws, cx| {
                                                                                                                ws.toggle_checked_file(path, cx);
                                                                                                            });
                                                                                                        } else {
                                                                                                            let q = filter_in.read(cx).value().trim().to_lowercase();
                                                                                                            let dir = m.full_path.clone();
                                                                                                            let checked = *val;
                                                                                                            ws_chk.update(cx, |ws, cx| {
                                                                                                                let mut paths = Vec::new();
                                                                                                                collect_leaf_paths_filtered(ws.file_tree(), &dir, &q, &mut paths);
                                                                                                                ws.set_checked_files_for_paths(paths, checked, cx);
                                                                                                            });
                                                                                                        }
                                                                                                    }
                                                                                                }
                                                                                            }),
                                                                                    )
                                                                                    .child(
                                                                                        Icon::new(icon_name)
                                                                                            .xsmall()
                                                                                            .text_color(if is_folder {
                                                                                                cx.theme().info
                                                                                            } else {
                                                                                                cx.theme().muted_foreground
                                                                                            }),
                                                                                    )
                                                                                    .child(
                                                                                        div()
                                                                                            .flex_1()
                                                                                            .min_w_0()
                                                                                            .text_xs()
                                                                                            .font_family(cx.theme().mono_font_family.clone())
                                                                                            .truncate()
                                                                                            .child(entry.item().label.clone()),
                                                                                    ),
                                                                            )
                                                                            .child(badge_el),
                                                                    )
                                                            })
                                                            .context_menu(move |_ix, entry, menu, _window, _cx| {
                                                                let Some(m) = node_meta_for_ctx.get(&entry.item().id).cloned() else {
                                                                    return menu;
                                                                };
                                                                if !m.is_leaf {
                                                                    return menu;
                                                                }
                                                                let ws_v = ws_for_ctx.clone();
                                                                let ws_t = ws_for_ctx.clone();
                                                                let ws_r = ws_for_ctx.clone();
                                                                let path_v = m.full_path.clone();
                                                                let path_t = m.full_path.clone();
                                                                let path_r = m.full_path.clone();
                                                                let range = range_for_ctx.clone();

                                                                menu.item(
                                                                    PopupMenuItem::new("View Diff")
                                                                        .icon(IconName::FileText)
                                                                        .on_click(move |_, _, cx| {
                                                                            ws_v.update(cx, |ws, cx| {
                                                                                ws.select_file(path_v.clone(), cx);
                                                                            });
                                                                        }),
                                                                )
                                                                .item(
                                                                    PopupMenuItem::new("Toggle Restore Selection")
                                                                        .icon(IconName::Check)
                                                                        .on_click(move |_, _, cx| {
                                                                            ws_t.update(cx, |ws, cx| {
                                                                                ws.toggle_checked_file(path_t.clone(), cx);
                                                                            });
                                                                        }),
                                                                )
                                                                .item(PopupMenuItem::separator())
                                                                .item(
                                                                    PopupMenuItem::new("Restore This File…")
                                                                        .icon(IconName::Undo2)
                                                                        .disabled(range.is_none())
                                                                        .on_click(move |_, window, cx| {
                                                                            if let Some((cfg, from, to)) = &range {
                                                                                open_undochange_confirm_dialog(
                                                                                    ws_r.clone(),
                                                                                    cfg.clone(),
                                                                                    *from,
                                                                                    *to,
                                                                                    vec![path_r.clone()],
                                                                                    window,
                                                                                    cx,
                                                                                );
                                                                            }
                                                                        }),
                                                                )
                                                            }),
                                                        )
                                                        .into_any_element()
                                                    }
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            resizable_panel().child(
                                v_flex()
                                    .id("diff-viewer-host")
                                    .test_support()
                                    .size_full()
                                    .min_h_0()
                                    .min_w_0()
                                    .bg(accents.surface_diff)
                                    .when_some(selected_file.clone(), |this, path| {
                                        this.child(
                                            h_flex()
                                                .w_full()
                                                .h(px(32.))
                                                .px_3p5()
                                                .gap_2()
                                                .items_center()
                                                .justify_between()
                                                .bg(accents.header_tint)
                                                .child(
                                                    h_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .gap_1p5()
                                                        .items_center()
                                                        .child(
                                                            Icon::new(LucideIconName::FileDiff)
                                                                .xsmall()
                                                                .text_color(cx.theme().primary),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .text_xs()
                                                                .font_family(
                                                                    cx.theme().mono_font_family.clone(),
                                                                )
                                                                .text_color(cx.theme().foreground)
                                                                .truncate()
                                                                .child(path.clone()),
                                                        ),
                                                )
                                                .child(
                                                    h_flex()
                                                        .gap_1p5()
                                                        .items_center()
                                                        .when_some(selected_change, |row, ch| {
                                                            row.child(change_badge(ch))
                                                        })
                                                        .child(
                                                            Clipboard::new("diff-copy-path")
                                                                .value(path)
                                                                .tooltip("Copy file path"),
                                                        ),
                                                ),
                                        )
                                    })
                                    .when(diff_truncated, |this| {
                                        this.child(
                                            Alert::warning(
                                                "diff-truncated-banner",
                                                format!(
                                                    "Diff truncated to 2 MiB ({} total bytes).",
                                                    snapper_core::format::format_size(diff_total_bytes as u64)
                                                ),
                                            )
                                            .banner()
                                            .small(),
                                        )
                                    })
                                    .child(if diff_loading {
                                        div()
                                            .flex_1()
                                            .min_h_0()
                                            .child(aether::skeleton_diff_lines(10))
                                            .into_any_element()
                                    } else if let Some(err) = diff_error {
                                        div()
                                            .flex_1()
                                            .p_3()
                                            .child(Alert::error("diff-load-error", err))
                                            .into_any_element()
                                    } else if diff_is_binary {
                                        div()
                                            .flex_1()
                                            .min_h_0()
                                            .child(aether::empty_state_card(
                                                "diff-binary-state",
                                                LucideIconName::Archive,
                                                cx.theme().warning,
                                                "Binary file modified",
                                                selected_file
                                                    .clone()
                                                    .unwrap_or_else(|| "<unknown>".to_string()),
                                                None,
                                                cx,
                                            ))
                                            .into_any_element()
                                    } else if let Some(raw) = &self.raw_fallback {
                                        div()
                                            .id("diff-raw-fallback")
                                            .flex_1()
                                            .min_h_0()
                                            .w_full()
                                            .p_3()
                                            .overflow_y_scrollbar()
                                            .child(
                                                v_flex()
                                                    .gap_2()
                                                    .when_some(self.parse_error.clone(), |this, err| {
                                                        this.child(
                                                            Alert::info(
                                                                "diff-raw-notice",
                                                                format!("Raw diff view ({err})"),
                                                            )
                                                            .small(),
                                                        )
                                                    })
                                                    .child(
                                                        div()
                                                            .font_family(
                                                                cx.theme().mono_font_family.clone(),
                                                            )
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(raw.clone()),
                                                    ),
                                            )
                                            .into_any_element()
                                    } else if selected_file.is_none() {
                                        div()
                                            .flex_1()
                                            .min_h_0()
                                            .child(aether::empty_state_card(
                                                "diff-unselected-state",
                                                LucideIconName::FileDiff,
                                                cx.theme().primary,
                                                "Select a changed file",
                                                "Pick any path from the tree above to inspect line-by-line diffs.",
                                                None,
                                                cx,
                                            ))
                                            .into_any_element()
                                    } else {
                                        div()
                                            .flex_1()
                                            .min_h_0()
                                            .min_w_0()
                                            .w_full()
                                            .child(
                                                Diff::new(&self.diff_state)
                                                    .line_number(true)
                                                    .syntax_highlight(true),
                                            )
                                            .into_any_element()
                                    }),
                            ),
                        ),
                ),
            )
            .into_any_element()
    }
}
