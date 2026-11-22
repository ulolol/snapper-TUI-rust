use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::command::{Command, CommandGroup, CommandItem, CommandState};
use gpui_kit::component::{IconName, WindowExt as _};
use gpui_kit::{App, AppContext as _, Entity, Focusable as _, ParentElement as _, Window, px};

use crate::actions::{
    CompareSnapshots, CycleTheme, NewSnapshot, Refresh, ToggleInspector, ToggleSidebar,
    ViewAnalytics, ViewSettings, ViewSnapshots,
};
use crate::state::{ActiveView, WorkspaceModel};
use crate::theme::{AppThemePreset, apply_theme_preset};
use crate::views::dialogs::compare::open_compare_dialog;
use crate::views::dialogs::confirm::open_cleanup_dialog;
use crate::views::dialogs::create::open_create_snapshot_dialog;

type PaletteActionFn = Box<dyn Fn(&mut Window, &mut App)>;
pub type ToggleCallback = Rc<dyn Fn(&mut Window, &mut App)>;
pub type ThemeSelectCallback = Rc<dyn Fn(AppThemePreset, &mut App)>;

pub fn open_command_palette(
    workspace: Entity<WorkspaceModel>,
    sidebar_open_toggle: ToggleCallback,
    inspector_open_toggle: ToggleCallback,
    on_set_theme: ThemeSelectCallback,
    window: &mut Window,
    cx: &mut App,
) {
    let configs = workspace.read(cx).configs().to_vec();
    let active_cfg = workspace.read(cx).filter().config.clone();

    let mut groups: Vec<(String, Vec<(CommandItem, PaletteActionFn)>)> = Vec::new();

    // Group 0: Snapshot Actions
    let ws1 = workspace.clone();
    let ws2 = workspace.clone();
    let ws3 = workspace.clone();
    let ws4 = workspace.clone();
    let toggle_sb = sidebar_open_toggle.clone();
    let toggle_ins = inspector_open_toggle.clone();
    groups.push((
        "Actions".to_string(),
        vec![
            (
                CommandItem::new()
                    .label("New Snapshot…")
                    .icon(IconName::Plus)
                    .action(Box::new(NewSnapshot)),
                Box::new(move |window, cx| {
                    open_create_snapshot_dialog(ws1.clone(), active_cfg.clone(), window, cx);
                }),
            ),
            (
                CommandItem::new()
                    .label("Compare Snapshots…")
                    .icon(IconName::Replace)
                    .action(Box::new(CompareSnapshots)),
                Box::new(move |window, cx| {
                    open_compare_dialog(ws2.clone(), None, window, cx);
                }),
            ),
            (
                CommandItem::new()
                    .label("Run Snapper Cleanup…")
                    .icon(IconName::RefreshCw),
                Box::new(move |window, cx| {
                    let cfg = ws3.read(cx).filter().config.clone();
                    open_cleanup_dialog(ws3.clone(), cfg, window, cx);
                }),
            ),
            (
                CommandItem::new()
                    .label("Refresh Snapshots")
                    .icon(IconName::RefreshCw)
                    .action(Box::new(Refresh)),
                Box::new(move |_, cx| {
                    ws4.update(cx, |ws, cx| ws.refresh(cx));
                }),
            ),
            (
                CommandItem::new()
                    .label("Toggle Sidebar")
                    .icon(IconName::PanelLeft)
                    .action(Box::new(ToggleSidebar)),
                Box::new(move |window, cx| {
                    (toggle_sb)(window, cx);
                }),
            ),
            (
                CommandItem::new()
                    .label("Toggle Inspector")
                    .icon(IconName::PanelRight)
                    .action(Box::new(ToggleInspector)),
                Box::new(move |window, cx| {
                    (toggle_ins)(window, cx);
                }),
            ),
        ],
    ));

    // Group 1: Views
    let ws_v1 = workspace.clone();
    let ws_v2 = workspace.clone();
    let ws_v3 = workspace.clone();
    groups.push((
        "Views".to_string(),
        vec![
            (
                CommandItem::new()
                    .label("View: Snapshots")
                    .icon(IconName::GalleryVerticalEnd)
                    .action(Box::new(ViewSnapshots)),
                Box::new(move |_, cx| {
                    ws_v1.update(cx, |ws, cx| ws.set_active_view(ActiveView::Snapshots, cx));
                }),
            ),
            (
                CommandItem::new()
                    .label("View: Storage Analytics")
                    .icon(IconName::ChartPie)
                    .action(Box::new(ViewAnalytics)),
                Box::new(move |_, cx| {
                    ws_v2.update(cx, |ws, cx| ws.set_active_view(ActiveView::Analytics, cx));
                }),
            ),
            (
                CommandItem::new()
                    .label("View: Config & Retention")
                    .icon(IconName::Settings)
                    .action(Box::new(ViewSettings)),
                Box::new(move |_, cx| {
                    ws_v3.update(cx, |ws, cx| ws.set_active_view(ActiveView::Settings, cx));
                }),
            ),
        ],
    ));

    // Group 2: Config Filters
    let mut cfg_entries: Vec<(CommandItem, PaletteActionFn)> = Vec::new();
    let ws_all = workspace.clone();
    cfg_entries.push((
        CommandItem::new()
            .label("Filter Config: All Configs")
            .icon(IconName::LayoutDashboard),
        Box::new(move |_, cx| {
            ws_all.update(cx, |ws, cx| {
                ws.set_active_view(ActiveView::Snapshots, cx);
                ws.set_config_filter(None, cx);
            });
        }),
    ));
    for cfg in configs {
        let name = cfg.name.clone();
        let ws_c = workspace.clone();
        cfg_entries.push((
            CommandItem::new()
                .label(format!("Filter Config: {} ({})", cfg.name, cfg.subvolume))
                .icon(IconName::HardDrive),
            Box::new(move |_, cx| {
                let target = name.clone();
                ws_c.update(cx, |ws, cx| {
                    ws.set_active_view(ActiveView::Snapshots, cx);
                    ws.set_config_filter(Some(target), cx);
                });
            }),
        ));
    }
    groups.push(("Configs".to_string(), cfg_entries));

    // Group 3: Themes
    let theme_entries: Vec<(CommandItem, PaletteActionFn)> = AppThemePreset::all()
        .iter()
        .copied()
        .map(|preset| {
            let setter = on_set_theme.clone();
            let item = CommandItem::new()
                .label(format!("Theme: {}", preset.label()))
                .icon(match preset {
                    AppThemePreset::CyberpunkDracula => IconName::Palette,
                    AppThemePreset::SystemDark => IconName::Moon,
                    AppThemePreset::SystemLight => IconName::Sun,
                })
                .action(Box::new(CycleTheme));
            let handler: PaletteActionFn = Box::new(move |_, cx| {
                apply_theme_preset(preset, cx);
                (setter)(preset, cx);
            });
            (item, handler)
        })
        .collect();
    groups.push(("Themes".to_string(), theme_entries));

    let cmd_state = cx.new(|cx| CommandState::new(window, cx));
    let initial_focus = Rc::new(Cell::new(true));

    // Store handlers by (section, row) for on_confirm lookup.
    let handlers: Rc<Vec<Vec<PaletteActionFn>>> = Rc::new(
        groups
            .iter()
            .map(|(_, items)| {
                // We rebuild handlers vector cleanly alongside CommandGroups below; move them in next.
                Vec::with_capacity(items.len())
            })
            .collect(),
    );
    let _ = handlers;

    let mut command_groups_data: Vec<(String, Vec<CommandItem>)> = Vec::with_capacity(groups.len());
    let mut callbacks: Vec<Vec<PaletteActionFn>> = Vec::with_capacity(groups.len());
    for (title, pairs) in groups {
        let mut items = Vec::with_capacity(pairs.len());
        let mut fns = Vec::with_capacity(pairs.len());
        for (item, cb) in pairs {
            items.push(item);
            fns.push(cb);
        }
        command_groups_data.push((title, items));
        callbacks.push(fns);
    }
    let callbacks = Rc::new(callbacks);
    let command_groups_data = Rc::new(command_groups_data);

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            cmd_state.focus_handle(cx).focus(window, cx);
        }

        let callbacks = callbacks.clone();
        let mut cmd = Command::new(&cmd_state)
            .bordered(false)
            .placeholder("Type a command, view, config, or theme…")
            .max_h(px(360.))
            .on_confirm(move |ix, window, cx| {
                window.close_dialog(cx);
                if let Some(handler) = callbacks.get(ix.section).and_then(|g| g.get(ix.row)) {
                    handler(window, cx);
                }
            })
            .on_cancel(|window, cx| {
                window.close_dialog(cx);
            });

        for (title, items) in command_groups_data.iter() {
            cmd = cmd.group(CommandGroup::new().label(title.clone()).items(items.clone()));
        }

        dialog
            .title("Command Palette")
            .width(px(560.))
            .child(cmd)
    });
}
