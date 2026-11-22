use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::form::{field, v_form};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::select::{Select, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, IndexPath, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Entity, Focusable as _, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use snapper_core::model::{CleanupAlgorithm, ModifySnapshotRequest, Snapshot};

use crate::state::WorkspaceModel;
use crate::views::dialogs::{ChoiceItem, parse_userdata_pairs};

pub fn open_modify_snapshot_dialog(
    workspace: Entity<WorkspaceModel>,
    snapshot: Snapshot,
    window: &mut Window,
    cx: &mut App,
) {
    let key = snapshot.key();
    let cleanup_items = vec![
        ChoiceItem::new("none", "none (clear cleanup algorithm)"),
        ChoiceItem::new("number", "number"),
        ChoiceItem::new("timeline", "timeline"),
        ChoiceItem::new("empty-pre-post", "empty-pre-post"),
    ];
    let selected_cleanup_ix = match snapshot.cleanup.as_deref() {
        Some("number") => 1,
        Some("timeline") => 2,
        Some("empty-pre-post") => 3,
        _ => 0,
    };

    let cleanup_select = cx.new(|cx| {
        SelectState::new(
            cleanup_items,
            Some(IndexPath::new(selected_cleanup_ix)),
            window,
            cx,
        )
    });

    let desc_input = cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("Snapshot description");
        state.set_value(snapshot.description.clone(), window, cx);
        state
    });

    let remaining_userdata: Vec<String> = snapshot
        .userdata
        .as_ref()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| k.as_str() != "important")
                .map(|(k, v)| format!("{k}={v}"))
                .collect()
        })
        .unwrap_or_default();
    let userdata_input = cx.new(|cx| {
        let mut state = InputState::new(window, cx).placeholder("key=val, pinned=yes");
        if !remaining_userdata.is_empty() {
            state.set_value(remaining_userdata.join(", "), window, cx);
        }
        state
    });

    let important_state = cx.new(|_| snapshot.is_important());
    let error_msg: Entity<Option<SharedString>> = cx.new(|_| None);
    let initial_focus = Rc::new(Cell::new(true));

    window.open_dialog(cx, move |dialog, window, cx| {
        if initial_focus.replace(false) {
            desc_input.focus_handle(cx).focus(window, cx);
        }

        let is_important = *important_state.read(cx);
        let current_err = error_msg.read(cx).clone();

        let ws = workspace.clone();
        let key = key.clone();
        let cln_sel = cleanup_select.clone();
        let desc_in = desc_input.clone();
        let ud_in = userdata_input.clone();
        let imp = important_state.clone();
        let err_state = error_msg.clone();

        dialog
            .title(format!("Modify Snapshot {} (#{})", key.config, key.number))
            .width(px(500.))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        v_form()
                            .child(
                                field()
                                    .label("Description")
                                    .child(Input::new(&desc_input).cleanable(true)),
                            )
                            .child(
                                field()
                                    .label("Cleanup Algorithm")
                                    .child(Select::new(&cleanup_select)),
                            )
                            .child(
                                field().label("Mark Important").child(
                                    h_flex().gap_2().items_center().child(
                                        Switch::new("dialog-modify-important")
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
                                    ),
                                ),
                            )
                            .child(
                                field()
                                    .label("Userdata")
                                    .child(Input::new(&userdata_input).cleanable(true)),
                            ),
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
                        Button::new("modify-cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                    )
                    .child(
                        Button::new("dialog-modify-submit")
                            .primary()
                            .label("Save Snapshot")
                            .on_click(move |_, window, cx| {
                                let description = Some(desc_in.read(cx).value().to_string());
                                let cleanup = cln_sel.read(cx).selected_value().map(|val| {
                                    if val == "none" {
                                        None
                                    } else {
                                        CleanupAlgorithm::from_cli_str(val)
                                    }
                                });

                                let mut userdata =
                                    match parse_userdata_pairs(ud_in.read(cx).value().as_ref()) {
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

                                let req = ModifySnapshotRequest {
                                    key: key.clone(),
                                    description,
                                    cleanup,
                                    userdata: Some(userdata),
                                };
                                ws.update(cx, |ws, cx| ws.modify_snapshot(req, cx));
                                window.close_dialog(cx);
                            }),
                    ),
            )
    });
}
