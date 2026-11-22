use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::form::{field, v_form};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme as _, IndexPath, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Entity, Focusable as _, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use snapper_core::model::Snapshot;

use crate::state::WorkspaceModel;
use crate::views::dialogs::ChoiceItem;

fn build_snapshot_choices(snapshots: &[Snapshot], config: &str) -> Vec<ChoiceItem> {
    let mut list: Vec<&Snapshot> = snapshots.iter().filter(|s| s.config == config).collect();
    list.sort_by_key(|s| s.number);
    let mut items: Vec<ChoiceItem> = list
        .into_iter()
        .map(|s| {
            let tag = if s.number == 0 {
                " (Current)".to_string()
            } else if s.description.is_empty() {
                String::new()
            } else {
                format!(" — {}", s.description)
            };
            ChoiceItem::new(s.number.to_string(), format!("#{}{tag}", s.number))
        })
        .collect();
    if items.is_empty() {
        items.push(ChoiceItem::new("0", "#0 (Current)"));
    }
    items
}

pub fn open_compare_dialog(
    workspace: Entity<WorkspaceModel>,
    initial_range: Option<(String, u32, u32)>,
    window: &mut Window,
    cx: &mut App,
) {
    let (configs, all_snapshots, default_cfg, default_from, default_to) = {
        let ws = workspace.read(cx);
        let mut cfg_items: Vec<ChoiceItem> = ws
            .configs()
            .iter()
            .map(|c| ChoiceItem::new(c.name.clone(), format!("{} ({})", c.name, c.subvolume)))
            .collect();
        if cfg_items.is_empty() {
            cfg_items.push(ChoiceItem::new("root", "root (/)"));
        }

        let (cfg, from, to) = initial_range
            .or_else(|| {
                ws.compare_range()
                    .map(|(c, f, t)| (c.to_string(), f, t))
            })
            .or_else(|| ws.selected_snapshot().map(|s| (s.config.clone(), s.number, 0)))
            .unwrap_or_else(|| (cfg_items[0].value.clone(), 0, 0));

        (cfg_items, ws.snapshots().to_vec(), cfg, from, to)
    };

    let cfg_ix = configs
        .iter()
        .position(|c| c.value == default_cfg)
        .unwrap_or(0);
    let initial_choices = build_snapshot_choices(&all_snapshots, &configs[cfg_ix].value);
    let from_ix = initial_choices
        .iter()
        .position(|c| c.value == default_from.to_string())
        .unwrap_or(0);
    let to_ix = initial_choices
        .iter()
        .position(|c| c.value == default_to.to_string())
        .unwrap_or_else(|| initial_choices.len().saturating_sub(1));

    let config_select =
        cx.new(|cx| SelectState::new(configs, Some(IndexPath::new(cfg_ix)), window, cx));
    let from_select = cx.new(|cx| {
        SelectState::new(initial_choices.clone(), Some(IndexPath::new(from_ix)), window, cx)
    });
    let to_select =
        cx.new(|cx| SelectState::new(initial_choices, Some(IndexPath::new(to_ix)), window, cx));
    let error_msg: Entity<Option<SharedString>> = cx.new(|_| None);

    let sub = window.subscribe(
        &config_select,
        cx,
        {
            let ws = workspace.clone();
            let from_sel = from_select.clone();
            let to_sel = to_select.clone();
            move |_, event: &SelectEvent<Vec<ChoiceItem>>, window, cx| {
                if let SelectEvent::Confirm(Some(cfg)) = event {
                    let choices = build_snapshot_choices(ws.read(cx).snapshots(), cfg);
                    from_sel.update(cx, |s, cx| s.set_items(choices.clone(), window, cx));
                    to_sel.update(cx, |s, cx| s.set_items(choices, window, cx));
                }
            }
        },
    );
    let sub_slot = Rc::new(sub);
    let initial_focus = Rc::new(Cell::new(true));

    window.open_dialog(cx, move |dialog, window, cx| {
        let _keep_sub = &sub_slot;
        if initial_focus.replace(false) {
            from_select.focus_handle(cx).focus(window, cx);
        }

        let current_err = error_msg.read(cx).clone();
        let ws = workspace.clone();
        let cfg_sel = config_select.clone();
        let from_sel = from_select.clone();
        let to_sel = to_select.clone();
        let err_state = error_msg.clone();

        dialog
            .title("Compare Snapshots")
            .width(px(520.))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Inspect file changes and diffs between any two snapshots in the same configuration (use #0 for the current live filesystem)."),
                    )
                    .child(
                        v_form()
                            .child(field().label("Configuration").child(Select::new(&config_select)))
                            .child(field().label("From Snapshot").child(Select::new(&from_select)))
                            .child(field().label("To Snapshot").child(Select::new(&to_select))),
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
                        Button::new("compare-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                    )
                    .child(
                        Button::new("dialog-compare-submit")
                            .primary()
                            .label("Compare")
                            .on_click(move |_, window, cx| {
                                let Some(config) = cfg_sel.read(cx).selected_value().cloned() else {
                                    return;
                                };
                                let from = from_sel
                                    .read(cx)
                                    .selected_value()
                                    .and_then(|v| v.parse::<u32>().ok())
                                    .unwrap_or(0);
                                let to = to_sel
                                    .read(cx)
                                    .selected_value()
                                    .and_then(|v| v.parse::<u32>().ok())
                                    .unwrap_or(0);
                                if from == to {
                                    err_state.update(cx, |e, cx| {
                                        *e = Some("Choose two different snapshot numbers to compare.".into());
                                        cx.notify();
                                    });
                                    return;
                                }
                                ws.update(cx, |ws, cx| ws.set_custom_compare(config, from, to, cx));
                                window.close_dialog(cx);
                            }),
                    ),
            )
    });
}
