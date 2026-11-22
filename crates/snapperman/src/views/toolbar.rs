use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use snapper_core::query::TypeFilter;

use crate::state::{WorkspaceEvent, WorkspaceModel};
use crate::views::dialogs::compare::open_compare_dialog;
use crate::views::dialogs::confirm::open_delete_confirm_dialog;
use crate::views::dialogs::create::open_create_snapshot_dialog;

fn type_filter_id(filter: TypeFilter) -> &'static str {
    match filter {
        TypeFilter::All => "toolbar-filter-all",
        TypeFilter::Single => "toolbar-filter-single",
        TypeFilter::PrePost => "toolbar-filter-pre-post",
        TypeFilter::Important => "toolbar-filter-important",
        TypeFilter::Pinned => "toolbar-filter-pinned",
    }
}

pub struct ToolbarView {
    workspace: Entity<WorkspaceModel>,
    search_input: Entity<InputState>,
    _subs: Vec<Subscription>,
}

impl ToolbarView {
    pub fn new(
        workspace: Entity<WorkspaceModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Filter #number, description, user, date…"));

        let input_sub = cx.subscribe_in(
            &search_input,
            window,
            |this, state, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = state.read(cx).value().to_string();
                    this.workspace.update(cx, |ws, cx| ws.set_query(query, cx));
                }
            },
        );

        let ws_sub = cx.subscribe(&workspace, |_, _, event: &WorkspaceEvent, cx| {
            if matches!(event, WorkspaceEvent::SnapshotsChanged) {
                cx.notify();
            }
        });

        Self {
            workspace,
            search_input,
            _subs: vec![input_sub, ws_sub],
        }
    }

    pub fn focus_search(&self, window: &mut Window, cx: &mut gpui_kit::App) {
        self.search_input.focus_handle(cx).focus(window, cx);
    }

    pub fn search_input(&self) -> &Entity<InputState> {
        &self.search_input
    }
}

impl Render for ToolbarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let active_type = ws.filter().type_filter;
        let active_config = ws.filter().config.clone();
        let checked_count = ws.checked_keys().len();
        let checked_pair = ws.same_config_checked_pair();
        let loading = ws.loading_snapshots();

        let ws_new = self.workspace.clone();
        let ws_refresh = self.workspace.clone();
        let ws_compare = self.workspace.clone();

        v_flex()
            .w_full()
            .bg(cx.theme().background)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .px_3()
                    .py_2()
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
                                div().w(px(280.)).min_w(px(160.)).child(
                                    Input::new(&self.search_input)
                                        .id("toolbar-search-input")
                                        .small()
                                        .cleanable(true)
                                        .prefix(
                                            Icon::new(IconName::Search)
                                                .small()
                                                .text_color(cx.theme().muted_foreground),
                                        ),
                                ),
                            )
                            .child(
                                h_flex().gap_1().items_center().children(
                                    TypeFilter::all_variants().iter().copied().map(|variant| {
                                        let is_active = active_type == variant;
                                        let ws_f = self.workspace.clone();
                                        let btn = Button::new(type_filter_id(variant))
                                            .small()
                                            .label(variant.label())
                                            .on_click(move |_, _, cx| {
                                                ws_f.update(cx, |ws, cx| {
                                                    ws.set_type_filter(variant, cx);
                                                });
                                            });
                                        if is_active { btn.primary() } else { btn.outline() }
                                    }),
                                ),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("toolbar-compare")
                                    .small()
                                    .outline()
                                    .icon(IconName::Replace)
                                    .label("Compare…")
                                    .on_click(move |_, window, cx| {
                                        open_compare_dialog(ws_compare.clone(), None, window, cx);
                                    }),
                            )
                            .child(
                                Button::new("toolbar-new-snapshot")
                                    .small()
                                    .primary()
                                    .icon(IconName::Plus)
                                    .label("New Snapshot…")
                                    .on_click({
                                        let cfg = active_config.clone();
                                        move |_, window, cx| {
                                            open_create_snapshot_dialog(
                                                ws_new.clone(),
                                                cfg.clone(),
                                                window,
                                                cx,
                                            );
                                        }
                                    }),
                            )
                            .child(
                                Button::new("toolbar-refresh")
                                    .small()
                                    .ghost()
                                    .icon(IconName::RefreshCw)
                                    .loading(loading)
                                    .tooltip("Refresh snapshots (Cmd/Ctrl+R / F5)")
                                    .on_click(move |_, _, cx| {
                                        ws_refresh.update(cx, |ws, cx| ws.refresh(cx));
                                    }),
                            ),
                    ),
            )
            .when(checked_count > 0, |this| {
                let ws_del = self.workspace.clone();
                let ws_clr = self.workspace.clone();
                let ws_cmp = self.workspace.clone();
                let checked_pair_for_btn = checked_pair.clone();

                this.child(
                    h_flex()
                        .w_full()
                        .px_3()
                        .py_1()
                        .gap_2()
                        .items_center()
                        .justify_between()
                        .bg(cx.theme().accent)
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().accent_foreground)
                                .child(format!("{checked_count} snapshot(s) selected")),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Button::new("toolbar-batch-compare")
                                        .xsmall()
                                        .outline()
                                        .icon(IconName::Replace)
                                        .label("Compare Selected")
                                        .disabled(checked_pair.is_none())
                                        .tooltip(if checked_pair.is_some() {
                                            "Compare the two checked snapshots"
                                        } else {
                                            "Check exactly 2 snapshots from the same config to compare"
                                        })
                                        .on_click(move |_, _, cx| {
                                            if let Some((cfg, from, to)) = &checked_pair_for_btn {
                                                ws_cmp.update(cx, |ws, cx| {
                                                    ws.set_custom_compare(
                                                        cfg.clone(),
                                                        *from,
                                                        *to,
                                                        cx,
                                                    );
                                                });
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("toolbar-batch-delete")
                                        .xsmall()
                                        .danger()
                                        .icon(IconName::Delete)
                                        .label(format!("Delete {checked_count}…"))
                                        .on_click(move |_, window, cx| {
                                            let targets = ws_del
                                                .read(cx)
                                                .checked_snapshots()
                                                .into_iter()
                                                .cloned()
                                                .collect();
                                            open_delete_confirm_dialog(
                                                ws_del.clone(),
                                                targets,
                                                window,
                                                cx,
                                            );
                                        }),
                                )
                                .child(
                                    Button::new("toolbar-batch-clear")
                                        .xsmall()
                                        .ghost()
                                        .label("Clear")
                                        .on_click(move |_, _, cx| {
                                            ws_clr.update(cx, |ws, cx| ws.clear_checked(cx));
                                        }),
                                ),
                        ),
                )
            })
    }
}
