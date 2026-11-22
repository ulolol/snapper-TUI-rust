use std::collections::BTreeMap;

use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use snapper_core::model::ConfigSetting;

use crate::state::{WorkspaceEvent, WorkspaceModel};
use crate::views::dialogs::confirm::open_cleanup_dialog;

fn setting_group_name(key: &str) -> &'static str {
    if matches!(key, "SUBVOLUME" | "FSTYPE" | "QGROUP") {
        "Identity (Read-Only)"
    } else if key.starts_with("TIMELINE_") {
        "Timeline Schedule & Retention"
    } else if key.starts_with("NUMBER_") {
        "Number Cleanup Policy"
    } else if key.starts_with("EMPTY_PRE_POST_") || key.starts_with("SPACE_") || key.starts_with("FREE_") {
        "Empty Pre-Post & Disk Quotas"
    } else if matches!(
        key,
        "ALLOW_USERS" | "ALLOW_GROUPS" | "SYNC_ACL" | "BACKGROUND_COMPARISON"
    ) {
        "Access Control & Background Tasks"
    } else {
        "Other Settings"
    }
}

fn is_boolean_setting(key: &str, value: &str) -> bool {
    matches!(value, "yes" | "no")
        && (key.starts_with("TIMELINE_CREATE")
            || key.starts_with("TIMELINE_CLEANUP")
            || key.starts_with("NUMBER_CLEANUP")
            || key.starts_with("EMPTY_PRE_POST_CLEANUP")
            || matches!(key, "SYNC_ACL" | "BACKGROUND_COMPARISON"))
}

pub struct SettingsView {
    workspace: Entity<WorkspaceModel>,
    draft_values: BTreeMap<String, String>,
    input_states: BTreeMap<String, Entity<InputState>>,
    _subs: Vec<Subscription>,
    _input_subs: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(
        workspace: Entity<WorkspaceModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ws_sub = cx.subscribe_in(
            &workspace,
            window,
            |this, _, event: &WorkspaceEvent, window, cx| {
                if matches!(event, WorkspaceEvent::ConfigSettingsChanged) {
                    this.rebuild_inputs(window, cx);
                    cx.notify();
                } else if matches!(event, WorkspaceEvent::SnapshotsChanged) {
                    cx.notify();
                }
            },
        );

        let mut view = Self {
            workspace,
            draft_values: BTreeMap::new(),
            input_states: BTreeMap::new(),
            _subs: vec![ws_sub],
            _input_subs: Vec::new(),
        };
        view.rebuild_inputs(window, cx);
        view
    }

    fn rebuild_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.workspace.read(cx).config_settings().to_vec();
        self.draft_values.clear();
        self.input_states.clear();
        self._input_subs.clear();

        for setting in settings {
            self.draft_values
                .insert(setting.key.clone(), setting.value.clone());
            if !setting.is_readonly() && !is_boolean_setting(&setting.key, &setting.value) {
                let key = setting.key.clone();
                let val = setting.value.clone();
                let input = cx.new(|cx| {
                    let mut st = InputState::new(window, cx).placeholder("value");
                    st.set_value(val, window, cx);
                    st
                });
                let sub = cx.subscribe_in(
                    &input,
                    window,
                    move |this, st, event: &InputEvent, _, cx| {
                        if matches!(event, InputEvent::Change) {
                            let new_val = st.read(cx).value().to_string();
                            this.draft_values.insert(key.clone(), new_val);
                            cx.notify();
                        }
                    },
                );
                self.input_states.insert(setting.key, input);
                self._input_subs.push(sub);
            }
        }
    }

    fn dirty_settings(&self, cx: &gpui_kit::App) -> Vec<ConfigSetting> {
        self.workspace
            .read(cx)
            .config_settings()
            .iter()
            .filter(|orig| !orig.is_readonly())
            .filter_map(|orig| {
                let draft = self.draft_values.get(&orig.key)?;
                if draft != &orig.value {
                    Some(ConfigSetting {
                        key: orig.key.clone(),
                        value: draft.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = self.workspace.read(cx);
        let configs = ws.configs().to_vec();
        let active_cfg = ws
            .settings_config()
            .map(ToOwned::to_owned)
            .or_else(|| configs.first().map(|c| c.name.clone()));
        let loading = ws.settings_loading();
        let error = ws.settings_error().map(ToOwned::to_owned);
        let settings = ws.config_settings().to_vec();
        let dirty = self.dirty_settings(cx);
        let dirty_count = dirty.len();

        let ws_reload = self.workspace.clone();
        let ws_cleanup = self.workspace.clone();
        let ws_save = self.workspace.clone();
        let cfg_for_reload = active_cfg.clone();
        let cfg_for_cleanup = active_cfg.clone();
        let cfg_for_save = active_cfg.clone();

        // Group settings cleanly while preserving deterministic display order.
        let group_order = [
            "Identity (Read-Only)",
            "Timeline Schedule & Retention",
            "Number Cleanup Policy",
            "Empty Pre-Post & Disk Quotas",
            "Access Control & Background Tasks",
            "Other Settings",
        ];
        let mut grouped: BTreeMap<&'static str, Vec<ConfigSetting>> = BTreeMap::new();
        for s in &settings {
            grouped
                .entry(setting_group_name(&s.key))
                .or_default()
                .push(s.clone());
        }

        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().background)
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .id("settings-view")
                    .test_support()
                    .w_full()
                    .p_5()
                    .gap_4()
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .justify_between()
                            .items_center()
                            .gap_3()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_bold()
                                            .text_color(cx.theme().foreground)
                                            .child("Config & Retention Manager"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Inspect and tune Snapper retention quotas (`get-config` / `set-config`)."),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Button::new("settings-run-cleanup")
                                            .small()
                                            .outline()
                                            .icon(IconName::RefreshCw)
                                            .label("Run Cleanup…")
                                            .on_click(move |_, window, cx| {
                                                open_cleanup_dialog(
                                                    ws_cleanup.clone(),
                                                    cfg_for_cleanup.clone(),
                                                    window,
                                                    cx,
                                                );
                                            }),
                                    )
                                    .child(
                                        Button::new("settings-reload")
                                            .small()
                                            .outline()
                                            .label("Reload")
                                            .loading(loading)
                                            .disabled(cfg_for_reload.is_none())
                                            .on_click(move |_, _, cx| {
                                                if let Some(cfg) = &cfg_for_reload {
                                                    ws_reload.update(cx, |ws, cx| {
                                                        ws.load_config_settings(cfg.clone(), cx);
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new("settings-save")
                                            .small()
                                            .primary()
                                            .icon(IconName::Check)
                                            .label(if dirty_count > 0 {
                                                format!("Save ({dirty_count})")
                                            } else {
                                                "Save Changes".to_string()
                                            })
                                            .disabled(dirty_count == 0 || cfg_for_save.is_none())
                                            .on_click(move |_, _, cx| {
                                                if let Some(cfg) = &cfg_for_save {
                                                    ws_save.update(cx, |ws, cx| {
                                                        ws.save_config_settings(
                                                            cfg.clone(),
                                                            dirty.clone(),
                                                            cx,
                                                        );
                                                    });
                                                }
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_2()
                            .items_center()
                            .children(configs.into_iter().map(|cfg| {
                                let selected = active_cfg.as_deref() == Some(cfg.name.as_str());
                                let ws_pick = self.workspace.clone();
                                let name = cfg.name.clone();
                                let btn_id: SharedString =
                                    format!("settings-config-{}", cfg.name).into();
                                let btn = Button::new(btn_id)
                                    .small()
                                    .label(format!("{} ({})", cfg.name, cfg.subvolume))
                                    .on_click(move |_, _, cx| {
                                        ws_pick.update(cx, |ws, cx| {
                                            ws.load_config_settings(name.clone(), cx);
                                        });
                                    });
                                if selected { btn.primary() } else { btn.outline() }
                            })),
                    )
                    .when_some(error, |this, err| {
                        this.child(Alert::error("settings-error", err))
                    })
                    .when(loading && settings.is_empty(), |this| {
                        this.child(
                            div()
                                .p_6()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Loading Snapper configuration…"),
                        )
                    })
                    .children(group_order.into_iter().filter_map(|group_title| {
                        let items = grouped.remove(group_title)?;
                        Some(
                            v_flex()
                                .w_full()
                                .p_4()
                                .gap_3()
                                .rounded(cx.theme().radius_lg)
                                .bg(cx.theme().group_box)
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(cx.theme().foreground)
                                        .child(group_title),
                                )
                                .children(items.into_iter().map(|setting| {
                                    let readonly = setting.is_readonly();
                                    let draft = self
                                        .draft_values
                                        .get(&setting.key)
                                        .cloned()
                                        .unwrap_or_else(|| setting.value.clone());
                                    let changed = draft != setting.value;

                                    let control = if readonly {
                                        div()
                                            .font_family(cx.theme().mono_font_family.clone())
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(setting.value.clone())
                                            .into_any_element()
                                    } else if is_boolean_setting(&setting.key, &setting.value) {
                                        let checked = draft == "yes";
                                        let key_for_sw = setting.key.clone();
                                        let view = cx.entity().clone();
                                        let sw_id: SharedString =
                                            format!("setting-switch-{}", setting.key).into();
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(Switch::new(sw_id).checked(checked).on_click(
                                                move |next, _, cx| {
                                                    let val = if *next { "yes" } else { "no" };
                                                    view.update(cx, |this, cx| {
                                                        this.draft_values.insert(
                                                            key_for_sw.clone(),
                                                            val.to_string(),
                                                        );
                                                        cx.notify();
                                                    });
                                                },
                                            ))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_family(cx.theme().mono_font_family.clone())
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(draft),
                                            )
                                            .into_any_element()
                                    } else if let Some(input) = self.input_states.get(&setting.key) {
                                        div()
                                            .w(px(260.))
                                            .child(
                                                Input::new(input)
                                                    .id(SharedString::from(format!(
                                                        "setting-input-{}",
                                                        setting.key
                                                    )))
                                                    .small(),
                                            )
                                            .into_any_element()
                                    } else {
                                        div().child(draft).into_any_element()
                                    };

                                    h_flex()
                                        .w_full()
                                        .py_1()
                                        .gap_4()
                                        .items_center()
                                        .justify_between()
                                        .border_t_1()
                                        .border_color(cx.theme().border.opacity(0.4))
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .font_family(
                                                            cx.theme().mono_font_family.clone(),
                                                        )
                                                        .text_xs()
                                                        .font_semibold()
                                                        .text_color(cx.theme().foreground)
                                                        .child(setting.key.clone()),
                                                )
                                                .when(changed, |this| {
                                                    this.child(
                                                        Tag::warning().xsmall().outline().child("edited"),
                                                    )
                                                }),
                                        )
                                        .child(control)
                                })),
                        )
                    })),
            )
    }
}
