use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::DiffMode;
use gpui_kit::component::notification::{Notification, NotificationType};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, TitleBar, WindowExt as _,
    h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};
use snapper_core::format::format_size;

use crate::actions::{
    CompareSnapshots, CycleTheme, FocusSearch, NewSnapshot, NextChange, OpenCommandPalette,
    PreviousChange, Refresh, ToggleDiffMode, ToggleInspector, ToggleSidebar, ViewAnalytics,
    ViewSettings, ViewSnapshots,
};
use crate::state::{ActiveView, AppPrefs, ToastLevel, WorkspaceEvent, WorkspaceModel};
use crate::theme::{AppThemePreset, ThemeAccents, apply_theme_preset};
use crate::views::aether;
use crate::views::analytics::AnalyticsView;
use crate::views::command_palette::open_command_palette;
use crate::views::dialogs::auth::open_auth_dialog;
use crate::views::dialogs::compare::open_compare_dialog;
use crate::views::dialogs::create::open_create_snapshot_dialog;
use crate::views::inspector::InspectorView;
use crate::views::settings::SettingsView;
use crate::views::sidebar::SidebarView;
use crate::views::table::SnapshotTableView;
use crate::views::toolbar::ToolbarView;

pub struct SnapperShell {
    workspace: Entity<WorkspaceModel>,
    prefs: AppPrefs,
    persist_prefs: bool,
    inspector_open: bool,
    auth_dialog_open: Rc<Cell<bool>>,
    focus_handle: FocusHandle,

    sidebar: Entity<SidebarView>,
    toolbar: Entity<ToolbarView>,
    table: Entity<SnapshotTableView>,
    inspector: Entity<InspectorView>,
    analytics: Entity<AnalyticsView>,
    settings: Entity<SettingsView>,
    _obs: Subscription,
    _sub: Subscription,
}

impl Focusable for SnapperShell {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SnapperShell {
    pub fn new(
        workspace: Entity<WorkspaceModel>,
        prefs: AppPrefs,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_persistence(workspace, prefs, true, window, cx)
    }

    pub fn new_with_persistence(
        workspace: Entity<WorkspaceModel>,
        prefs: AppPrefs,
        persist_prefs: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let sidebar = cx.new(|cx| SidebarView::new(workspace.clone(), prefs.sidebar_collapsed, cx));
        let toolbar = cx.new(|cx| ToolbarView::new(workspace.clone(), window, cx));
        let table = cx.new(|cx| SnapshotTableView::new(workspace.clone(), window, cx));
        let inspector = cx.new(|cx| InspectorView::new(workspace.clone(), window, cx));
        if prefs.split_diff {
            inspector.update(cx, |ins, cx| {
                ins.diff_pane().update(cx, |dp, cx| {
                    dp.set_diff_mode(DiffMode::Split, cx);
                });
            });
        }
        let analytics = cx.new(|cx| AnalyticsView::new(workspace.clone(), cx));
        let settings = cx.new(|cx| SettingsView::new(workspace.clone(), window, cx));

        let auth_dialog_open = Rc::new(Cell::new(false));
        let auth_guard_sub = auth_dialog_open.clone();
        let ws_for_auth_sub = workspace.clone();

        let obs = cx.observe(&workspace, |_, _, cx| cx.notify());
        let sub = cx.subscribe_in(
            &workspace,
            window,
            move |_, _, event: &WorkspaceEvent, window, cx| match event {
                WorkspaceEvent::Toast {
                    level,
                    title,
                    message,
                } => {
                    let ntype = match level {
                        ToastLevel::Info => NotificationType::Info,
                        ToastLevel::Success => NotificationType::Success,
                        ToastLevel::Warning => NotificationType::Warning,
                        ToastLevel::Error => NotificationType::Error,
                    };
                    window.push_notification(
                        Notification::new()
                            .with_type(ntype)
                            .title(title.clone())
                            .message(message.clone()),
                        cx,
                    );
                }
                WorkspaceEvent::AuthenticationRequired { .. } => {
                    open_auth_dialog(ws_for_auth_sub.clone(), auth_guard_sub.clone(), window, cx);
                }
                _ => {
                    cx.notify();
                }
            },
        );

        if workspace.read(cx).auth_required() {
            let ws_defer = workspace.clone();
            let guard_defer = auth_dialog_open.clone();
            window.defer(cx, move |window, cx| {
                open_auth_dialog(ws_defer, guard_defer, window, cx);
            });
        }

        Self {
            workspace,
            prefs,
            persist_prefs,
            inspector_open: true,
            auth_dialog_open,
            focus_handle,
            sidebar,
            toolbar,
            table,
            inspector,
            analytics,
            settings,
            _obs: obs,
            _sub: sub,
        }
    }

    fn save_prefs_if_enabled(&self) {
        if self.persist_prefs {
            self.prefs.save();
        }
    }

    pub fn workspace(&self) -> &Entity<WorkspaceModel> {
        &self.workspace
    }

    pub fn toolbar(&self) -> &Entity<ToolbarView> {
        &self.toolbar
    }

    pub fn table(&self) -> &Entity<SnapshotTableView> {
        &self.table
    }

    pub fn inspector(&self) -> &Entity<InspectorView> {
        &self.inspector
    }

    pub fn theme_preset(&self) -> AppThemePreset {
        self.prefs.theme
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.prefs.sidebar_collapsed = !self.prefs.sidebar_collapsed;
        let collapsed = self.prefs.sidebar_collapsed;
        self.sidebar.update(cx, |sb, cx| sb.set_collapsed(collapsed, cx));
        self.save_prefs_if_enabled();
        cx.notify();
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector_open = !self.inspector_open;
        cx.notify();
    }

    pub fn set_theme_preset(&mut self, preset: AppThemePreset, cx: &mut Context<Self>) {
        self.prefs.theme = preset;
        apply_theme_preset(preset, cx);
        self.save_prefs_if_enabled();
        cx.notify();
    }

    pub fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        let next = self.prefs.theme.next();
        self.set_theme_preset(next, cx);
    }

    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell_sb = cx.entity().clone();
        let shell_ins = cx.entity().clone();
        let shell_theme = cx.entity().clone();
        open_command_palette(
            self.workspace.clone(),
            Rc::new(move |_, cx| {
                shell_sb.update(cx, |s, cx| s.toggle_sidebar(cx));
            }),
            Rc::new(move |_, cx| {
                shell_ins.update(cx, |s, cx| s.toggle_inspector(cx));
            }),
            Rc::new(move |preset, cx| {
                shell_theme.update(cx, |s, cx| s.set_theme_preset(preset, cx));
            }),
            window,
            cx,
        );
    }
}

impl Render for SnapperShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let active_view = ws.active_view();
        let backend_label = ws.backend_label();
        let is_mock = ws.backend().is_mock();
        let is_dbus = backend_label.to_lowercase().contains("d-bus");
        let total_count = ws.snapshots().len();
        let filtered_count = ws.filtered_indices().len();
        let hydrating = ws.hydrating_space();
        let busy_label = ws.busy_label().map(ToOwned::to_owned);
        let active_cfg_label = ws
            .filter()
            .config
            .as_deref()
            .unwrap_or("All Configs")
            .to_string();
        let settings_cfg_label = ws.settings_config().map(ToOwned::to_owned);

        let total_tracked: u64 = ws
            .filtered_indices()
            .iter()
            .filter_map(|&idx| ws.snapshots().get(idx))
            .filter_map(|s| s.used_space)
            .sum();

        let sidebar_collapsed = self.prefs.sidebar_collapsed;
        let inspector_open = self.inspector_open;
        let theme_preset = self.prefs.theme;
        let auth_required = ws.auth_required();
        let is_authenticated = ws.is_authenticated();

        let view_title = match active_view {
            ActiveView::Snapshots => "Snapshots",
            ActiveView::Analytics => "Storage Analytics",
            ActiveView::Settings => "Config & Retention",
        };
        let scope_crumb: Option<String> = match active_view {
            ActiveView::Snapshots => Some(active_cfg_label.clone()),
            ActiveView::Analytics => None,
            ActiveView::Settings => settings_cfg_label,
        };

        let conduit_color = if auth_required {
            cx.theme().warning
        } else if hydrating || busy_label.is_some() {
            cx.theme().info
        } else if is_mock {
            cx.theme().warning
        } else {
            cx.theme().success
        };
        let accents = ThemeAccents::from_theme(cx.theme());

        v_flex()
            .id("snapperman-shell")
            .key_context("Snapperman")
            .track_focus(&self.focus_handle)
            .relative()
            .overflow_hidden()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &Refresh, _, cx| {
                this.workspace.update(cx, |ws, cx| ws.refresh(cx));
            }))
            .on_action(cx.listener(|this, _: &NewSnapshot, window, cx| {
                let cfg = this.workspace.read(cx).filter().config.clone();
                open_create_snapshot_dialog(this.workspace.clone(), cfg, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CompareSnapshots, window, cx| {
                open_compare_dialog(this.workspace.clone(), None, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleInspector, _, cx| {
                this.toggle_inspector(cx);
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.toolbar.update(cx, |tb, cx| tb.focus_search(window, cx));
            }))
            .on_action(cx.listener(|this, _: &OpenCommandPalette, window, cx| {
                this.open_palette(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CycleTheme, _, cx| {
                this.cycle_theme(cx);
            }))
            .on_action(cx.listener(|this, _: &ViewSnapshots, _, cx| {
                this.workspace
                    .update(cx, |ws, cx| ws.set_active_view(ActiveView::Snapshots, cx));
            }))
            .on_action(cx.listener(|this, _: &ViewAnalytics, _, cx| {
                this.workspace
                    .update(cx, |ws, cx| ws.set_active_view(ActiveView::Analytics, cx));
            }))
            .on_action(cx.listener(|this, _: &ViewSettings, _, cx| {
                this.workspace
                    .update(cx, |ws, cx| ws.set_active_view(ActiveView::Settings, cx));
            }))
            .on_action(cx.listener(|this, _: &NextChange, _, cx| {
                this.inspector.update(cx, |ins, cx| {
                    ins.diff_pane().update(cx, |dp, cx| dp.next_change(cx));
                });
            }))
            .on_action(cx.listener(|this, _: &PreviousChange, _, cx| {
                this.inspector.update(cx, |ins, cx| {
                    ins.diff_pane().update(cx, |dp, cx| dp.previous_change(cx));
                });
            }))
            .on_action(cx.listener(|this, _: &ToggleDiffMode, _, cx| {
                let split_now = this.inspector.update(cx, |ins, cx| {
                    ins.diff_pane()
                        .update(cx, |dp, cx| dp.toggle_diff_mode(cx) == DiffMode::Split)
                });
                this.prefs.split_diff = split_now;
                this.save_prefs_if_enabled();
            }))
            .child(aether::theme_backdrop(theme_preset, cx))
            .child(
                TitleBar::new().border_b_0().bg(accents.chrome_bg).child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .pr_2()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Button::new("titlebar-toggle-sidebar")
                                        .xsmall()
                                        .ghost()
                                        .rounded(aether::pill_radius())
                                        .icon(if sidebar_collapsed {
                                            IconName::PanelLeftOpen
                                        } else {
                                            IconName::PanelLeftClose
                                        })
                                        .tooltip("Toggle Sidebar (Cmd/Ctrl+B)")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_sidebar(cx);
                                        })),
                                )
                                .child(aether::brand_sigil(px(18.), cx))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_bold()
                                        .text_color(cx.theme().foreground)
                                        .child("snapperman"),
                                )
                                .child(
                                    Icon::new(LucideIconName::ChevronRight)
                                        .xsmall()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_medium()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(view_title),
                                )
                                .when_some(scope_crumb, |this, cfg| {
                                    this.child(
                                        Icon::new(LucideIconName::ChevronRight)
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(cx.theme().accent_foreground)
                                            .child(cfg),
                                    )
                                })
                                .child(if is_mock {
                                    Tag::warning()
                                        .xsmall()
                                        .rounded_full()
                                        .border_0()
                                        .child(
                                            h_flex()
                                                .gap_1()
                                                .items_center()
                                                .child(Icon::new(LucideIconName::Cpu).xsmall())
                                                .child("Mock"),
                                        )
                                } else if is_dbus {
                                    Tag::success()
                                        .xsmall()
                                        .rounded_full()
                                        .border_0()
                                        .child(
                                            h_flex()
                                                .gap_1()
                                                .items_center()
                                                .child(Icon::new(LucideIconName::Zap).xsmall())
                                                .child("D-Bus"),
                                        )
                                } else {
                                    Tag::info()
                                        .xsmall()
                                        .rounded_full()
                                        .border_0()
                                        .child(
                                            h_flex()
                                                .gap_1()
                                                .items_center()
                                                .child(Icon::new(LucideIconName::Activity).xsmall())
                                                .child("CLI"),
                                        )
                                }),
                        )
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .when(auth_required, |this| {
                                    this.child(
                                        Button::new("titlebar-auth-unlock")
                                            .xsmall()
                                            .warning()
                                            .rounded(aether::pill_radius())
                                            .icon(Icon::new(LucideIconName::ShieldAlert))
                                            .label("Authenticate")
                                            .tooltip("Unlock administrator privileges for Snapper")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                open_auth_dialog(
                                                    this.workspace.clone(),
                                                    this.auth_dialog_open.clone(),
                                                    window,
                                                    cx,
                                                );
                                            })),
                                    )
                                })
                                .when(!auth_required && is_authenticated, |this| {
                                    this.child(
                                        Tag::success()
                                            .xsmall()
                                            .rounded_full()
                                            .border_0()
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .items_center()
                                                    .child(Icon::new(LucideIconName::ShieldCheck).xsmall())
                                                    .child("Unlocked"),
                                            ),
                                    )
                                    .child(
                                        Button::new("titlebar-auth-lock")
                                            .xsmall()
                                            .ghost()
                                            .rounded(aether::pill_radius())
                                            .icon(Icon::new(LucideIconName::LockOpen))
                                            .label("Lock")
                                            .tooltip("Clear cached administrator credentials from memory")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.workspace
                                                    .update(cx, |ws, cx| ws.lock_session(cx));
                                            })),
                                    )
                                })
                                .child(
                                    Button::new("titlebar-command-palette")
                                        .xsmall()
                                        .secondary()
                                        .rounded(aether::pill_radius())
                                        .icon(IconName::Search)
                                        .label("Commands…")
                                        .tooltip("Open Command Palette (Cmd/Ctrl+K)")
                                        .child(aether::shortcut_kbd("cmd-k", "ctrl-k", cx))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_palette(window, cx);
                                        })),
                                )
                                .when(active_view == ActiveView::Snapshots, |this| {
                                    this.child(
                                        Button::new("titlebar-toggle-inspector")
                                            .xsmall()
                                            .ghost()
                                            .rounded(aether::pill_radius())
                                            .icon(IconName::PanelRight)
                                            .tooltip("Toggle Inspector (Cmd/Ctrl+I)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.toggle_inspector(cx);
                                            })),
                                    )
                                })
                                .child(
                                    Button::new("titlebar-cycle-theme")
                                        .xsmall()
                                        .ghost()
                                        .rounded(aether::pill_radius())
                                        .icon(Icon::new(theme_preset.icon()))
                                        .label(theme_preset.short_label())
                                        .tooltip(
                                            "Cycle Theme (Cmd/Ctrl+T): Aetherpunk Citadel -> Arcanepunk Enclave -> Aether Parchment -> Cyberpunk Dracula -> System Dark -> System Light",
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cycle_theme(cx);
                                        })),
                                ),
                        ),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .w_full()
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .h_full()
                            .map(|body| match active_view {
                                ActiveView::Snapshots => body
                                    .flex()
                                    .flex_col()
                                    .child(self.toolbar.clone())
                                    .child(
                                        div().flex_1().min_h_0().min_w_0().w_full().child(
                                            h_resizable("main-content-split")
                                                .with_handle_appearance(aether::soft_resize_handle())
                                                .child(
                                                    resizable_panel().child(self.table.clone()),
                                                )
                                                .child(
                                                    resizable_panel()
                                                        .size(px(460.))
                                                        .size_range(px(320.)..px(760.))
                                                        .fixed()
                                                        .visible(inspector_open)
                                                        .child(self.inspector.clone()),
                                                ),
                                        ),
                                    )
                                    .into_any_element(),
                                ActiveView::Analytics => body.child(self.analytics.clone()).into_any_element(),
                                ActiveView::Settings => body.child(self.settings.clone()).into_any_element(),
                            }),
                    ),
            )
            .child({
                let is_filtered = filtered_count != total_count;
                let count_text: SharedString = if !is_filtered {
                    format!("{total_count} snapshots").into()
                } else {
                    format!("{filtered_count} / {total_count} filtered").into()
                };
                let space_text: SharedString = if hydrating {
                    "Calculating space…".into()
                } else {
                    format!("Tracked: {}", format_size(total_tracked)).into()
                };
                let center_busy = busy_label.or_else(|| hydrating.then(|| "Hydrating space…".to_string()));

                let mut bar = StatusBar::new()
                    .border_t_0()
                    .bg(accents.chrome_bg)
                    .px_3()
                    .left(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().size(px(7.)).rounded_full().bg(conduit_color))
                            .child(div().text_xs().text_color(cx.theme().foreground).child(backend_label))
                            .child(aether::status_dot(cx))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        Icon::new(LucideIconName::Layers)
                                            .xsmall()
                                            .text_color(if is_filtered {
                                                cx.theme().info
                                            } else {
                                                cx.theme().muted_foreground
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(if is_filtered {
                                                cx.theme().accent_foreground
                                            } else {
                                                cx.theme().muted_foreground
                                            })
                                            .child(count_text),
                                    ),
                            )
                            .child(aether::status_dot(cx))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        Icon::new(LucideIconName::HardDrive)
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(div().text_xs().child(space_text)),
                            ),
                    )
                    .right(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        Icon::new(LucideIconName::HardDrive)
                                            .xsmall()
                                            .text_color(cx.theme().primary),
                                    )
                                    .child(div().text_xs().child(active_cfg_label)),
                            )
                            .child(aether::status_dot(cx))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        Icon::new(theme_preset.icon())
                                            .xsmall()
                                            .text_color(cx.theme().primary),
                                    )
                                    .child(div().text_xs().child(theme_preset.label())),
                            )
                            .child(aether::status_dot(cx))
                            .child(aether::shortcut_kbd("cmd-k", "ctrl-k", cx)),
                    );

                if let Some(busy) = center_busy {
                    bar = bar.child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Progress::new("status-hydration-progress")
                                    .xsmall()
                                    .loading(true)
                                    .w(px(56.)),
                            )
                            .child(div().text_xs().text_color(cx.theme().info).child(busy)),
                    );
                }
                bar
            })
    }
}
