use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{Ordering, compiler_fence};

use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::form::{field, v_form};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::{
    App, AppContext as _, Entity, Focusable as _, InteractiveElement as _, ParentElement as _,
    SharedString, Styled as _, Window, div, px,
};

use crate::state::WorkspaceModel;
use crate::views::aether;
use crate::views::dialogs::dialog_header_banner;

struct DialogOpenGuard(Rc<Cell<bool>>);

impl Drop for DialogOpenGuard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

#[derive(Clone)]
struct AuthDialogHandles {
    workspace: Entity<WorkspaceModel>,
    password_input: Entity<InputState>,
    busy: Entity<bool>,
    error_msg: Entity<Option<SharedString>>,
}

fn submit_password(handles: AuthDialogHandles, window: &mut Window, cx: &mut App) {
    if *handles.busy.read(cx) {
        return;
    }

    let password = handles.password_input.read(cx).value().to_string();
    if password.is_empty() {
        handles.error_msg.update(cx, |err, cx| {
            *err = Some("Please enter your sudo password.".into());
            cx.notify();
        });
        handles.password_input.focus_handle(cx).focus(window, cx);
        window.refresh();
        return;
    }

    // Clear the UI text buffer immediately before running sudo validation.
    handles
        .password_input
        .update(cx, |state, cx| state.set_value("", window, cx));
    handles.error_msg.update(cx, |err, cx| {
        *err = None;
        cx.notify();
    });
    handles.busy.update(cx, |b, cx| {
        *b = true;
        cx.notify();
    });
    window.refresh();

    let backend = handles.workspace.read(cx).backend().clone();
    let h = handles.clone();
    window
        .spawn(cx, async move |cx| {
            let mut pw_bytes = password.into_bytes();
            let res = cx
                .background_executor()
                .spawn(async move {
                    let result = match std::str::from_utf8(&pw_bytes) {
                        Ok(pw) => backend.authenticate_with_password(pw),
                        Err(_) => anyhow::bail!("Password must be valid UTF-8."),
                    };
                    pw_bytes.fill(0);
                    compiler_fence(Ordering::SeqCst);
                    result
                })
                .await;

            let _ = cx.update(|window, cx| {
                h.busy.update(cx, |b, cx| {
                    *b = false;
                    cx.notify();
                });
                match res {
                    Ok(()) => {
                        h.workspace.update(cx, |ws, cx| ws.on_authenticated(cx));
                        window.close_dialog(cx);
                    }
                    Err(err) => {
                        h.error_msg.update(cx, |e, cx| {
                            *e = Some(err.to_string().into());
                            cx.notify();
                        });
                        h.password_input.focus_handle(cx).focus(window, cx);
                        window.refresh();
                    }
                }
            });
        })
        .detach();
}

fn submit_pkexec(handles: AuthDialogHandles, window: &mut Window, cx: &mut App) {
    if *handles.busy.read(cx) {
        return;
    }

    handles.error_msg.update(cx, |err, cx| {
        *err = None;
        cx.notify();
    });
    handles.busy.update(cx, |b, cx| {
        *b = true;
        cx.notify();
    });
    window.refresh();

    let backend = handles.workspace.read(cx).backend().clone();
    let h = handles.clone();
    window
        .spawn(cx, async move |cx| {
            let res = cx
                .background_executor()
                .spawn(async move { backend.enable_pkexec_mode() })
                .await;

            let _ = cx.update(|window, cx| {
                h.busy.update(cx, |b, cx| {
                    *b = false;
                    cx.notify();
                });
                match res {
                    Ok(()) => {
                        h.password_input
                            .update(cx, |state, cx| state.set_value("", window, cx));
                        h.workspace.update(cx, |ws, cx| ws.on_authenticated(cx));
                        window.close_dialog(cx);
                    }
                    Err(err) => {
                        h.error_msg.update(cx, |e, cx| {
                            *e = Some(err.to_string().into());
                            cx.notify();
                        });
                        h.password_input.focus_handle(cx).focus(window, cx);
                        window.refresh();
                    }
                }
            });
        })
        .detach();
}

pub fn open_auth_dialog(
    workspace: Entity<WorkspaceModel>,
    open_guard: Rc<Cell<bool>>,
    window: &mut Window,
    cx: &mut App,
) {
    if open_guard.replace(true) {
        return;
    }

    let current_user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "current user".to_string());

    let password_input = cx.new(|cx| {
        InputState::new(window, cx)
            .masked(true)
            .placeholder("Enter sudo password…")
    });
    let busy = cx.new(|_| false);
    let error_msg: Entity<Option<SharedString>> = cx.new(|_| None);

    let handles = AuthDialogHandles {
        workspace,
        password_input: password_input.clone(),
        busy: busy.clone(),
        error_msg: error_msg.clone(),
    };

    let handles_for_enter = handles.clone();
    let sub = window.subscribe(
        &password_input,
        cx,
        move |_, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let h = handles_for_enter.clone();
                window.defer(cx, move |window, cx| {
                    submit_password(h, window, cx);
                });
            }
        },
    );

    let sub_slot = Rc::new(sub);
    let guard_token = Rc::new(DialogOpenGuard(open_guard.clone()));
    let guard_on_close = open_guard;
    let initial_focus = Rc::new(Cell::new(true));

    window.open_dialog(cx, move |dialog, window, cx| {
        let _keep_sub = &sub_slot;
        let _keep_guard = &guard_token;

        if initial_focus.replace(false) {
            password_input.focus_handle(cx).focus(window, cx);
        }

        let is_busy = *busy.read(cx);
        let current_err = error_msg.read(cx).clone();
        let handles_submit = handles.clone();
        let handles_pkexec = handles.clone();
        let pw_clear = password_input.clone();
        let guard_close = guard_on_close.clone();

        dialog
            .title("Administrator Authentication")
            .width(px(480.))
            .on_close(move |_, _, _| {
                guard_close.set(false);
            })
            .child(
                v_flex()
                    .gap_3()
                    .child(dialog_header_banner(
                        LucideIconName::KeyRound,
                        cx.theme().primary,
                        format!("Unlock Snapper access for '{current_user}'"),
                        "Validated once via sudo and kept in memory only for this session (cleared on Lock or exit).",
                        cx,
                    ))
                    .child(
                        v_form().child(
                            field().label("Password").required(true).child(
                                Input::new(&password_input)
                                    .id("dialog-auth-password")
                                    .mask_toggle(),
                            ),
                        ),
                    )
                    .children(current_err.map(|err| {
                        div()
                            .id("dialog-auth-error")
                            .test_support()
                            .text_sm()
                            .text_color(cx.theme().danger)
                            .child(err)
                    })),
            )
            .footer(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("dialog-auth-pkexec")
                            .ghost()
                            .rounded(aether::pill_radius())
                            .label("Use Polkit (pkexec)")
                            .disabled(is_busy)
                            .on_click(move |_, window, cx| {
                                submit_pkexec(handles_pkexec.clone(), window, cx);
                            }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("dialog-auth-cancel")
                                    .rounded(aether::pill_radius())
                                    .label("Cancel")
                                    .on_click(move |_, window, cx| {
                                        pw_clear.update(cx, |s, cx| s.set_value("", window, cx));
                                        window.close_dialog(cx);
                                    }),
                            )
                            .child(
                                Button::new("dialog-auth-submit")
                                    .primary()
                                    .rounded(aether::pill_radius())
                                    .label(if is_busy {
                                        "Authenticating…"
                                    } else {
                                        "Authenticate"
                                    })
                                    .loading(is_busy)
                                    .disabled(is_busy)
                                    .on_click(move |_, window, cx| {
                                        submit_password(handles_submit.clone(), window, cx);
                                    }),
                            ),
                    ),
            )
    });
}
