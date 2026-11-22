use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
    px,
};
use snapper_core::query::TypeFilter;

use crate::state::{WorkspaceEvent, WorkspaceModel};
use crate::theme::ThemeAccents;
use crate::views::aether;
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

        let ws_sub = cx.subscribe_in(
            &workspace,
            window,
            |this, ws, event: &WorkspaceEvent, window, cx| {
                if matches!(event, WorkspaceEvent::SnapshotsChanged) {
                    if ws.read(cx).filter().query.is_empty()
                        && !this.search_input.read(cx).value().is_empty()
                    {
                        this.search_input
                            .update(cx, |st, cx| st.set_value("", window, cx));
                    }
                    cx.notify();
                }
            },
        );

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
        let has_query = !ws.filter().query.trim().is_empty();
        let has_active_filter = has_query || active_type != TypeFilter::All;
        let checked_count = ws.checked_keys().len();
        let checked_pair = ws.same_config_checked_pair();
        let loading = ws.loading_snapshots();

        let ws_new = self.workspace.clone();
        let ws_refresh = self.workspace.clone();
        let ws_compare = self.workspace.clone();
        let ws_reset = self.workspace.clone();
        let search_for_reset = self.search_input.clone();

        let accents = ThemeAccents::from_theme(cx.theme());

        v_flex()
            .id("toolbar-card")
            .w_full()
            .bg(accents.surface_panel)
            .child(
                h_flex()
                    .w_full()
                    .px_3p5()
                    .py_2p5()
                    .gap_2p5()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w(px(320.))
                            .gap_2()
                            .flex_wrap()
                            .items_center()
                            .child(
                                div().flex_1().min_w(px(150.)).max_w(px(250.)).child(
                                    Input::new(&self.search_input)
                                        .id("toolbar-search-input")
                                        .small()
                                        .cleanable(true)
                                        .prefix(
                                            Icon::new(IconName::Search)
                                                .small()
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                        .when(!has_query, |this| {
                                            this.suffix(aether::shortcut_kbd("cmd-f", "ctrl-f", cx))
                                        }),
                                ),
                            )
                            .child(
                                h_flex()
                                    .p(px(3.))
                                    .gap(px(2.))
                                    .items_center()
                                    .rounded_full()
                                    .bg(accents.sunken_bg)
                                    .children(TypeFilter::all_variants().iter().copied().map(|variant| {
                                        let is_active = active_type == variant;
                                        let ws_f = self.workspace.clone();
                                        let btn = Button::new(type_filter_id(variant))
                                            .xsmall()
                                            .rounded(aether::pill_radius())
                                            .label(variant.label())
                                            .on_click(move |_, _, cx| {
                                                ws_f.update(cx, |ws, cx| {
                                                    ws.set_type_filter(variant, cx);
                                                });
                                            });
                                        if is_active {
                                            btn.secondary()
                                        } else {
                                            btn.ghost()
                                        }
                                    })),
                            )
                            .when(has_active_filter, |this| {
                                this.child(
                                    Button::new("toolbar-reset-filters")
                                        .xsmall()
                                        .ghost()
                                        .rounded(aether::pill_radius())
                                        .icon(Icon::new(LucideIconName::X))
                                        .label("Reset")
                                        .tooltip("Clear search query and type filter")
                                        .on_click(move |_, window, cx| {
                                            search_for_reset.update(cx, |st, cx| {
                                                st.set_value("", window, cx);
                                            });
                                            ws_reset.update(cx, |ws, cx| {
                                                ws.set_query(String::new(), cx);
                                                ws.set_type_filter(TypeFilter::All, cx);
                                            });
                                        }),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("toolbar-compare")
                                    .small()
                                    .secondary()
                                    .rounded(aether::pill_radius())
                                    .icon(Icon::new(LucideIconName::GitCompareArrows))
                                    .label("Compare…")
                                    .tooltip("Compare any two snapshots (Cmd/Ctrl+D)")
                                    .on_click(move |_, window, cx| {
                                        open_compare_dialog(ws_compare.clone(), None, window, cx);
                                    }),
                            )
                            .child(
                                Button::new("toolbar-new-snapshot")
                                    .small()
                                    .primary()
                                    .rounded(aether::pill_radius())
                                    .icon(IconName::Plus)
                                    .label("New Snapshot…")
                                    .tooltip("Create snapshot (Cmd/Ctrl+N)")
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
                                    .rounded(aether::pill_radius())
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
                        .mx_3p5()
                        .mb_2p5()
                        .px_3p5()
                        .py_1p5()
                        .gap_2()
                        .items_center()
                        .justify_between()
                        .rounded_full()
                        .bg(cx.theme().accent)
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(
                                    Icon::new(LucideIconName::Layers)
                                        .xsmall()
                                        .text_color(cx.theme().accent_foreground),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_semibold()
                                        .text_color(cx.theme().accent_foreground)
                                        .child(format!("{checked_count} snapshot(s) selected")),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Button::new("toolbar-batch-compare")
                                        .xsmall()
                                        .secondary()
                                        .rounded(aether::pill_radius())
                                        .icon(Icon::new(LucideIconName::GitCompareArrows))
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
                                        .rounded(aether::pill_radius())
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
                                        .rounded(aether::pill_radius())
                                        .icon(Icon::new(LucideIconName::X))
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
