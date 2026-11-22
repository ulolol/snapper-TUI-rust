use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::sidebar::{
    Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarItem, SidebarMenuItem,
};
use gpui_kit::component::{
    ActiveTheme as _, Collapsible, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    App, Context, ElementId, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use snapper_core::format::format_size;

use crate::state::{ActiveView, WorkspaceEvent, WorkspaceModel};
use crate::theme::ThemeAccents;
use crate::views::aether;
use crate::views::dialogs::confirm::open_cleanup_dialog;

#[derive(Clone)]
struct NamedSidebarMenuItem {
    id: SharedString,
    item: SidebarMenuItem,
    collapsed: bool,
}

impl NamedSidebarMenuItem {
    fn new(id: impl Into<SharedString>, item: SidebarMenuItem) -> Self {
        Self {
            id: id.into(),
            item,
            collapsed: false,
        }
    }
}

impl Collapsible for NamedSidebarMenuItem {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl SidebarItem for NamedSidebarMenuItem {
    fn render(
        self,
        _id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        self.item.collapsed(self.collapsed).render(self.id, window, cx)
    }
}

#[derive(Clone)]
struct NamedSidebarMenu {
    items: Vec<NamedSidebarMenuItem>,
    collapsed: bool,
}

impl NamedSidebarMenu {
    fn new(items: Vec<NamedSidebarMenuItem>) -> Self {
        Self {
            items,
            collapsed: false,
        }
    }
}

impl Collapsible for NamedSidebarMenu {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl SidebarItem for NamedSidebarMenu {
    fn render(
        self,
        _id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        v_flex()
            .gap_1()
            .children(self.items.into_iter().map(|item| {
                let id = item.id.clone();
                item.collapsed(self.collapsed)
                    .render(id, window, cx)
                    .into_any_element()
            }))
    }
}

#[derive(Clone)]
enum SidebarSection {
    Group(SidebarGroup<NamedSidebarMenu>),
    FiligreeRule { collapsed: bool },
}

impl Collapsible for SidebarSection {
    fn is_collapsed(&self) -> bool {
        match self {
            Self::Group(g) => g.is_collapsed(),
            Self::FiligreeRule { collapsed } => *collapsed,
        }
    }

    fn collapsed(self, collapsed: bool) -> Self {
        match self {
            Self::Group(g) => Self::Group(g.collapsed(collapsed)),
            Self::FiligreeRule { .. } => Self::FiligreeRule { collapsed },
        }
    }
}

impl SidebarItem for SidebarSection {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        match self {
            Self::Group(g) => g.render(id, window, cx).into_any_element(),
            Self::FiligreeRule { collapsed } => {
                if collapsed {
                    div().into_any_element()
                } else {
                    div()
                        .px_3()
                        .py_0p5()
                        .child(aether::filigree_wave(cx))
                        .into_any_element()
                }
            }
        }
    }
}

pub struct SidebarView {
    workspace: Entity<WorkspaceModel>,
    collapsed: bool,
    _sub: Subscription,
}

impl SidebarView {
    pub fn new(workspace: Entity<WorkspaceModel>, collapsed: bool, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&workspace, |_, _, event: &WorkspaceEvent, cx| {
            if matches!(
                event,
                WorkspaceEvent::SnapshotsChanged | WorkspaceEvent::SelectionChanged
            ) {
                cx.notify();
            }
        });
        Self {
            workspace,
            collapsed,
            _sub: sub,
        }
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    pub fn set_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        self.collapsed = collapsed;
        cx.notify();
    }
}

impl Render for SidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let active_config = ws.filter().config.clone();
        let active_view = ws.active_view();
        let total_count = ws.snapshots().len();
        let configs = ws.configs().to_vec();
        let total_space: u64 = ws.snapshots().iter().filter_map(|s| s.used_space).sum();
        let hydrating = ws.hydrating_space();

        let mut config_items = Vec::with_capacity(configs.len() + 1);
        let ws_all = self.workspace.clone();
        let all_active = active_view == ActiveView::Snapshots && active_config.is_none();
        config_items.push(NamedSidebarMenuItem::new(
            "sidebar-config-all",
            SidebarMenuItem::new("All Configs")
                .py_1()
                .px_2()
                .icon(IconName::LayoutDashboard)
                .active(all_active)
                .suffix(move |_, cx| {
                    aether::count_pill("sidebar-config-all-count", total_count, 999, all_active, cx)
                })
                .on_click(move |_, _, cx| {
                    ws_all.update(cx, |ws, cx| {
                        ws.set_active_view(ActiveView::Snapshots, cx);
                        ws.set_config_filter(None, cx);
                    });
                }),
        ));

        for cfg in &configs {
            let count = ws.config_snapshot_count(&cfg.name);
            let cfg_name = cfg.name.clone();
            let ws_cfg = self.workspace.clone();
            let selected = active_view == ActiveView::Snapshots
                && active_config.as_deref() == Some(cfg.name.as_str());
            let label: SharedString = cfg.name.clone().into();
            let item_id: SharedString = format!("sidebar-config-{}", cfg.name).into();
            let count_id: SharedString = format!("sidebar-config-{}-count", cfg.name).into();

            config_items.push(NamedSidebarMenuItem::new(
                item_id,
                SidebarMenuItem::new(label)
                    .py_1()
                    .px_2()
                    .icon(IconName::HardDrive)
                    .active(selected)
                    .suffix(move |_, cx| {
                        aether::count_pill(count_id.clone(), count, 999, selected, cx)
                    })
                    .on_click(move |_, _, cx| {
                        let target = cfg_name.clone();
                        ws_cfg.update(cx, |ws, cx| {
                            ws.set_active_view(ActiveView::Snapshots, cx);
                            ws.set_config_filter(Some(target), cx);
                        });
                    }),
            ));
        }

        let view_items = vec![
            {
                let ws_v = self.workspace.clone();
                NamedSidebarMenuItem::new(
                    "sidebar-nav-snapshots",
                    SidebarMenuItem::new("Snapshots")
                        .icon(Icon::new(LucideIconName::Layers))
                        .active(active_view == ActiveView::Snapshots)
                        .on_click(move |_, _, cx| {
                            ws_v.update(cx, |ws, cx| ws.set_active_view(ActiveView::Snapshots, cx));
                        }),
                )
            },
            {
                let ws_v = self.workspace.clone();
                NamedSidebarMenuItem::new(
                    "sidebar-nav-analytics",
                    SidebarMenuItem::new("Storage Analytics")
                        .icon(Icon::new(LucideIconName::ChartPie))
                        .active(active_view == ActiveView::Analytics)
                        .on_click(move |_, _, cx| {
                            ws_v.update(cx, |ws, cx| ws.set_active_view(ActiveView::Analytics, cx));
                        }),
                )
            },
            {
                let ws_v = self.workspace.clone();
                let current_cfg = active_config
                    .clone()
                    .or_else(|| configs.first().map(|c| c.name.clone()));
                NamedSidebarMenuItem::new(
                    "sidebar-nav-settings",
                    SidebarMenuItem::new("Config & Retention")
                        .icon(Icon::new(LucideIconName::SlidersHorizontal))
                        .active(active_view == ActiveView::Settings)
                        .on_click(move |_, _, cx| {
                            let cfg = current_cfg.clone();
                            ws_v.update(cx, |ws, cx| {
                                ws.set_active_view(ActiveView::Settings, cx);
                                if let Some(c) = cfg {
                                    ws.load_config_settings(c, cx);
                                }
                            });
                        }),
                )
            },
        ];

        let collapsed = self.collapsed;
        let ws_cleanup = self.workspace.clone();
        let cleanup_cfg = active_config.clone();
        let cfg_count = configs.len();
        let vault_space: SharedString = if hydrating {
            "Calculating…".into()
        } else {
            format_size(total_space).into()
        };

        let accents = ThemeAccents::from_theme(cx.theme());

        div()
            .flex_shrink_0()
            .h_full()
            .overflow_hidden()
            .bg(accents.surface_sidebar)
            .child(
                Sidebar::new("app-sidebar")
                    .collapsed(collapsed)
                    .border_0()
                    .bg(cx.theme().transparent)
                    .header(
                        SidebarHeader::new().child(if collapsed {
                            h_flex()
                                .w_full()
                                .justify_center()
                                .py_1()
                                .child(aether::brand_sigil(px(20.), cx))
                                .into_any_element()
                        } else {
                            v_flex()
                                .w_full()
                                .gap_1p5()
                                .px_2p5()
                                .py_2()
                                .rounded(cx.theme().radius)
                                .bg(accents.sunken_bg)
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(aether::brand_sigil(px(18.), cx))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_semibold()
                                                .text_color(cx.theme().sidebar_foreground)
                                                .child("Snapshot Vault"),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .items_center()
                                        .text_xs()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{total_count} snaps · {cfg_count} cfg"))
                                        .child(
                                            div()
                                                .text_color(cx.theme().sidebar_accent_foreground)
                                                .child(vault_space),
                                        ),
                                )
                                .into_any_element()
                        }),
                    )
                    .child(SidebarSection::Group(
                        SidebarGroup::new("Configs").child(NamedSidebarMenu::new(config_items)),
                    ))
                    .child(SidebarSection::FiligreeRule { collapsed })
                    .child(SidebarSection::Group(
                        SidebarGroup::new("Views").child(NamedSidebarMenu::new(view_items)),
                    ))
                    .footer(
                        SidebarFooter::new().child(
                            Button::new("sidebar-run-cleanup")
                                .secondary()
                                .small()
                                .rounded(aether::pill_radius())
                                .w_full()
                                .icon(Icon::new(LucideIconName::Archive))
                                .when(!collapsed, |b| b.label("Run Cleanup…"))
                                .tooltip("Run Snapper cleanup algorithm")
                                .on_click(move |_, window, cx| {
                                    open_cleanup_dialog(
                                        ws_cleanup.clone(),
                                        cleanup_cfg.clone(),
                                        window,
                                        cx,
                                    );
                                }),
                        ),
                    ),
            )
    }
}
