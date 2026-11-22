use std::collections::BTreeMap;

use gpui_kit::assets::IconName as LucideIconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use snapper_core::model::ConfigSetting;

use crate::state::{WorkspaceEvent, WorkspaceModel};
use crate::views::aether;
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

fn setting_group_icon(group: &str) -> LucideIconName {
    match group {
        "Identity (Read-Only)" => LucideIconName::ShieldCheck,
        "Timeline Schedule & Retention" => LucideIconName::Clock,
        "Number Cleanup Policy" => LucideIconName::Layers,
        "Empty Pre-Post & Disk Quotas" => LucideIconName::HardDrive,
        "Access Control & Background Tasks" => LucideIconName::KeyRound,
        _ => LucideIconName::SlidersHorizontal,
    }
}

fn setting_description(key: &str) -> Option<&'static str> {
    match key {
        "SUBVOLUME" => Some("Target filesystem mount path managed by this Snapper profile."),
        "FSTYPE" => Some("Underlying snapshot filesystem driver (e.g. btrfs or lvm)."),
        "QGROUP" => Some("Optional Btrfs quota group (qgroup) tracking exclusive snapshot space."),
        "TIMELINE_CREATE" => Some("Automatically capture hourly snapshots via systemd/cron timer."),
        "TIMELINE_CLEANUP" => Some("Prune older timeline snapshots according to bucket limits."),
        "TIMELINE_MIN_AGE" => Some("Minimum age in seconds before timeline snapshots can be pruned."),
        "TIMELINE_LIMIT_HOURLY" => Some("Maximum hourly timeline snapshots retained per bucket."),
        "TIMELINE_LIMIT_DAILY" => Some("Maximum daily timeline snapshots retained."),
        "TIMELINE_LIMIT_WEEKLY" => Some("Maximum weekly timeline snapshots retained."),
        "TIMELINE_LIMIT_MONTHLY" => Some("Maximum monthly timeline snapshots retained."),
        "TIMELINE_LIMIT_QUARTERLY" => Some("Maximum quarterly timeline snapshots retained."),
        "TIMELINE_LIMIT_YEARLY" => Some("Maximum yearly timeline snapshots retained."),
        "NUMBER_CLEANUP" => Some("Enable automatic count-based cleanup for numbered snapshots."),
        "NUMBER_MIN_AGE" => Some("Minimum age in seconds before numbered snapshots become eligible for cleanup."),
        "NUMBER_LIMIT" => Some("Maximum total snapshots retained under the number algorithm (or min-max range)."),
        "NUMBER_LIMIT_IMPORTANT" => Some("Maximum snapshots marked `important=yes` retained by number cleanup."),
        "EMPTY_PRE_POST_CLEANUP" => Some("Automatically remove pre/post pairs that contain zero filesystem changes."),
        "EMPTY_PRE_POST_MIN_AGE" => Some("Minimum age in seconds before empty pre/post pairs are cleaned up."),
        "SPACE_LIMIT" => Some("Fraction of filesystem capacity snapshots may occupy before cleanup trims them."),
        "FREE_LIMIT" => Some("Minimum fraction of free filesystem space reserved during cleanup."),
        "ALLOW_USERS" => Some("Space-separated non-root usernames allowed to operate on this config."),
        "ALLOW_GROUPS" => Some("Space-separated UNIX groups permitted to access this config."),
        "SYNC_ACL" => Some("Synchronize `.snapshots` directory ACLs for entries in ALLOW_USERS/GROUPS."),
        "BACKGROUND_COMPARISON" => Some("Pre-compute file comparisons asynchronously in the background."),
        _ => None,
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

    pub fn discard_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rebuild_inputs(window, cx);
        cx.notify();
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
        let ws_save_banner = self.workspace.clone();
        let cfg_for_reload = active_cfg.clone();
        let cfg_for_cleanup = active_cfg.clone();
        let cfg_for_save = active_cfg.clone();
        let cfg_for_save_banner = active_cfg.clone();
        let dirty_for_banner = dirty.clone();

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
            .overflow_y_scrollbar()
            .child(
                v_flex()
                    .id("settings-view")
                    .test_support()
                    .w_full()
                    .p_5()
                    .gap_4()
                    .child(
                        aether::soft_card("settings-header-card", cx)
                            .w_full()
                            .child(
                                v_flex()
                                    .w_full()
                                    .p_4()
                                    .gap_3()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .flex_wrap()
                                            .justify_between()
                                            .items_center()
                                            .gap_3()
                                            .child(
                                                h_flex()
                                                    .gap_2p5()
                                                    .items_center()
                                                    .child(aether::brand_sigil(px(24.), cx))
                                                    .child(
                                                        v_flex()
                                                            .gap_0p5()
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
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        Button::new("settings-run-cleanup")
                                                            .small()
                                                            .secondary()
                                                            .rounded(aether::pill_radius())
                                                            .icon(Icon::new(LucideIconName::Archive))
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
                                                            .secondary()
                                                            .rounded(aether::pill_radius())
                                                            .icon(IconName::RefreshCw)
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
                                                            .rounded(aether::pill_radius())
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
                                                    .rounded(aether::pill_radius())
                                                    .icon(IconName::HardDrive)
                                                    .label(format!("{} ({})", cfg.name, cfg.subvolume))
                                                    .on_click(move |_, _, cx| {
                                                        ws_pick.update(cx, |ws, cx| {
                                                            ws.load_config_settings(name.clone(), cx);
                                                        });
                                                    });
                                                if selected { btn.primary() } else { btn.secondary() }
                                            })),
                                    ),
                            ),
                    )
                    .when(dirty_count > 0, |this| {
                        this.child(
                            h_flex()
                                .id("settings-dirty-banner")
                                .w_full()
                                .px_4()
                                .py_2p5()
                                .gap_3()
                                .items_center()
                                .justify_between()
                                .rounded(cx.theme().radius_lg)
                                .bg(cx.theme().warning.opacity(0.14))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Icon::new(LucideIconName::TriangleAlert)
                                                .small()
                                                .text_color(cx.theme().warning),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_semibold()
                                                .text_color(cx.theme().foreground)
                                                .child(format!(
                                                    "{dirty_count} unsaved setting change(s) staged for '{}'.",
                                                    cfg_for_save_banner.as_deref().unwrap_or("config")
                                                )),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            Button::new("settings-discard")
                                                .xsmall()
                                                .secondary()
                                                .rounded(aether::pill_radius())
                                                .icon(Icon::new(LucideIconName::RotateCcw))
                                                .label("Discard")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.discard_changes(window, cx);
                                                })),
                                        )
                                        .child(
                                            Button::new("settings-banner-save")
                                                .xsmall()
                                                .primary()
                                                .rounded(aether::pill_radius())
                                                .icon(IconName::Check)
                                                .label(format!("Save ({dirty_count})"))
                                                .disabled(cfg_for_save_banner.is_none())
                                                .on_click(move |_, _, cx| {
                                                    if let Some(cfg) = &cfg_for_save_banner {
                                                        ws_save_banner.update(cx, |ws, cx| {
                                                            ws.save_config_settings(
                                                                cfg.clone(),
                                                                dirty_for_banner.clone(),
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }),
                                        ),
                                ),
                        )
                    })
                    .when_some(error, |this, err| {
                        this.child(Alert::error("settings-error", err))
                    })
                    .when(loading && settings.is_empty(), |this| {
                        this.child(aether::skeleton_settings_cards(3))
                    })
                    .children(group_order.into_iter().filter_map(|group_title| {
                        let items = grouped.remove(group_title)?;
                        let group_id: SharedString = format!(
                            "settings-group-{}",
                            group_title.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "-")
                        )
                        .into();
                        let group_icon = setting_group_icon(group_title);

                        Some(
                            aether::soft_card(group_id, cx)
                                .w_full()
                                .child(
                                    v_flex()
                                        .p_4()
                                        .gap_3()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(aether::icon_medallion(
                                                    group_icon,
                                                    cx.theme().primary,
                                                    28.,
                                                ))
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_semibold()
                                                        .text_color(cx.theme().foreground)
                                                        .child(group_title),
                                                ),
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
                                                .py_2()
                                                .gap_4()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .gap_0p5()
                                                        .child(
                                                            h_flex()
                                                                .gap_2()
                                                                .items_center()
                                                                .child(
                                                                    div()
                                                                        .font_family(
                                                                            cx.theme()
                                                                                .mono_font_family
                                                                                .clone(),
                                                                        )
                                                                        .text_xs()
                                                                        .font_semibold()
                                                                        .text_color(
                                                                            cx.theme().foreground,
                                                                        )
                                                                        .child(setting.key.clone()),
                                                                )
                                                                .when(changed, |this| {
                                                                    this.child(
                                                                        Tag::warning()
                                                                            .xsmall()
                                                                            .rounded_full()
                                                                            .border_0()
                                                                            .child("edited"),
                                                                    )
                                                                }),
                                                        )
                                                        .when_some(
                                                            setting_description(&setting.key),
                                                            |this, desc| {
                                                                this.child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .child(desc),
                                                                )
                                                            },
                                                        ),
                                                )
                                                .child(control)
                                        })),
                                ),
                        )
                    })),
            )
    }
}
