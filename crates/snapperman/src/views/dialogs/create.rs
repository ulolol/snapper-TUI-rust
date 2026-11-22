use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::form::{field, v_form};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, IndexPath, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Entity, Focusable as _, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use snapper_core::model::{CleanupAlgorithm, CreateSnapshotRequest, SnapshotCreateType};

use crate::state::WorkspaceModel;
use crate::views::dialogs::{ChoiceItem, parse_userdata_pairs};

pub fn open_create_snapshot_dialog(
    workspace: Entity<WorkspaceModel>,
    default_config: Option<String>,
    window: &mut Window,
    cx: &mut App,
) {
    let (configs, latest_pre) = {
        let ws = workspace.read(cx);
        let mut items: Vec<ChoiceItem> = ws
            .configs()
            .iter()
            .map(|c| ChoiceItem::new(c.name.clone(), format!("{} ({})", c.name, c.subvolume)))
            .collect();
        if items.is_empty() {
            items.push(ChoiceItem::new("root", "root (/)"));
        }
        let target_cfg = default_config
            .clone()
            .unwrap_or_else(|| items[0].value.clone());
        let pre_num = ws
            .snapshots()
            .iter()
            .filter(|s| s.config == target_cfg && s.is_pre())
            .map(|s| s.number)
            .max();
        (items, pre_num)
    };

    let selected_cfg_ix = default_config
        .as_deref()
        .and_then(|cfg| configs.iter().position(|c| c.value == cfg))
        .unwrap_or(0);

    let type_items = vec![
        ChoiceItem::new("single", "single — Standalone point-in-time snapshot"),
        ChoiceItem::new("pre", "pre — Before a change (pair later with post)"),
        ChoiceItem::new("post", "post — After a change (pairs with a pre #)"),
    ];
    let cleanup_items = vec![
        ChoiceItem::new("number", "number (retain by max count)"),
        ChoiceItem::new("timeline", "timeline (retain by time buckets)"),
        ChoiceItem::new("empty-pre-post", "empty-pre-post"),
        ChoiceItem::new("none", "none (manual retention)"),
    ];

    let config_select =
        cx.new(|cx| SelectState::new(configs, Some(IndexPath::new(selected_cfg_ix)), window, cx));
    let type_select =
        cx.new(|cx| SelectState::new(type_items, Some(IndexPath::new(0)), window, cx));
    let cleanup_select =
        cx.new(|cx| SelectState::new(cleanup_items, Some(IndexPath::new(0)), window, cx));

    let pre_number_input = cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("e.g. 12");
        if let Some(num) = latest_pre {
            state.set_value(num.to_string(), window, cx);
        }
        state
    });
    let desc_input = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Before system upgrade"));
    let userdata_input =
        cx.new(|cx| InputState::new(window, cx).placeholder("optional key=val, ticket=42"));
    let important_state = cx.new(|_| false);
    let error_msg: Entity<Option<SharedString>> = cx.new(|_| None);

    // Refresh the window when type selection changes so the Pre-number field toggles immediately.
    let sub = window.subscribe(&type_select, cx, |_, _: &SelectEvent<Vec<ChoiceItem>>, window, _| {
        window.refresh();
    });
    let sub_slot = Rc::new(sub);
    let initial_focus = Rc::new(Cell::new(true));

    window.open_dialog(cx, move |dialog, window, cx| {
        let _keep_sub = &sub_slot;
        if initial_focus.replace(false) {
            desc_input.focus_handle(cx).focus(window, cx);
        }

        let is_post = type_select
            .read(cx)
            .selected_value()
            .is_some_and(|v| v == "post");
        let is_important = *important_state.read(cx);
        let current_err = error_msg.read(cx).clone();

        let ws = workspace.clone();
        let cfg_sel = config_select.clone();
        let typ_sel = type_select.clone();
        let cln_sel = cleanup_select.clone();
        let pre_in = pre_number_input.clone();
        let desc_in = desc_input.clone();
        let ud_in = userdata_input.clone();
        let imp = important_state.clone();
        let err_state = error_msg.clone();

        let mut form = v_form()
            .child(field().label("Configuration").child(Select::new(&config_select)))
            .child(field().label("Snapshot Type").child(Select::new(&type_select)));
        if is_post {
            form = form.child(
                field()
                    .label("Paired Pre #")
                    .required(true)
                    .child(Input::new(&pre_number_input).cleanable(true)),
            );
        }
        form = form
            .child(
                field()
                    .label("Description")
                    .required(true)
                    .child(Input::new(&desc_input).id("dialog-create-description").cleanable(true)),
            )
            .child(field().label("Cleanup Algorithm").child(Select::new(&cleanup_select)))
            .child(
                field().label("Mark Important").child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Switch::new("dialog-create-important")
                                .checked(is_important)
                                .on_click({
                                    let imp = important_state.clone();
                                    move |checked, _, cx| {
                                        imp.update(cx, |v, cx| {
                                            *v = *checked;
                                            cx.notify();
                                        });
                                    }
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Sets userdata `important=yes`"),
                        ),
                ),
            )
            .child(
                field()
                    .label("Extra Userdata")
                    .child(Input::new(&userdata_input).cleanable(true)),
            );

        dialog
            .title("Create Snapshot")
            .width(px(520.))
            .child(
                v_flex()
                    .gap_3()
                    .child(form)
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
                        Button::new("create-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                    )
                    .child(
                        Button::new("dialog-create-submit")
                            .primary()
                            .label("Create Snapshot")
                            .on_click(move |_, window, cx| {
                                let Some(config) = cfg_sel.read(cx).selected_value().cloned() else {
                                    err_state.update(cx, |e, cx| {
                                        *e = Some("Please select a configuration.".into());
                                        cx.notify();
                                    });
                                    return;
                                };

                                let description = desc_in.read(cx).value().trim().to_string();
                                if description.is_empty() {
                                    err_state.update(cx, |e, cx| {
                                        *e = Some("Description is required.".into());
                                        cx.notify();
                                    });
                                    return;
                                }

                                let type_str = typ_sel
                                    .read(cx)
                                    .selected_value()
                                    .cloned()
                                    .unwrap_or_else(|| "single".to_string());
                                let snapshot_type = match type_str.as_str() {
                                    "pre" => SnapshotCreateType::Pre,
                                    "post" => {
                                        let raw_pre = pre_in.read(cx).value().trim().to_string();
                                        let Ok(pre_number) = raw_pre.parse::<u32>() else {
                                            err_state.update(cx, |e, cx| {
                                                *e = Some("Post snapshot requires a numeric Pre #.".into());
                                                cx.notify();
                                            });
                                            return;
                                        };
                                        SnapshotCreateType::Post { pre_number }
                                    }
                                    _ => SnapshotCreateType::Single,
                                };

                                let cleanup = cln_sel.read(cx).selected_value().and_then(|val| {
                                    if val == "none" {
                                        None
                                    } else {
                                        CleanupAlgorithm::from_cli_str(val)
                                    }
                                });

                                let mut userdata = match parse_userdata_pairs(ud_in.read(cx).value().as_ref()) {
                                    Ok(map) => map,
                                    Err(err) => {
                                        err_state.update(cx, |e, cx| {
                                            *e = Some(err.into());
                                            cx.notify();
                                        });
                                        return;
                                    }
                                };
                                if *imp.read(cx) {
                                    userdata.insert("important".to_string(), "yes".to_string());
                                }

                                let req = CreateSnapshotRequest {
                                    config,
                                    description,
                                    snapshot_type,
                                    cleanup,
                                    userdata,
                                };
                                ws.update(cx, |ws, cx| ws.create_snapshot(req, cx));
                                window.close_dialog(cx);
                            }),
                    ),
            )
    });
}
