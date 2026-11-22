use gpui_kit::{App, KeyBinding, actions};

actions!(
    snapperman,
    [
        Refresh,
        NewSnapshot,
        ModifySelected,
        DeleteSelected,
        RollbackSelected,
        CompareSnapshots,
        ToggleSidebar,
        ToggleInspector,
        FocusSearch,
        OpenCommandPalette,
        CycleTheme,
        ViewSnapshots,
        ViewAnalytics,
        ViewSettings,
        NextChange,
        PreviousChange,
        ToggleDiffMode,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let is_mac = cx.platform_info().is_mac();
    cx.bind_keys([
        KeyBinding::new("f5", Refresh, None),
        KeyBinding::new(if is_mac { "cmd-r" } else { "ctrl-r" }, Refresh, None),
        KeyBinding::new(if is_mac { "cmd-n" } else { "ctrl-n" }, NewSnapshot, None),
        KeyBinding::new(if is_mac { "cmd-k" } else { "ctrl-k" }, OpenCommandPalette, None),
        KeyBinding::new(if is_mac { "cmd-f" } else { "ctrl-f" }, FocusSearch, None),
        KeyBinding::new(if is_mac { "cmd-b" } else { "ctrl-b" }, ToggleSidebar, None),
        KeyBinding::new(if is_mac { "cmd-i" } else { "ctrl-i" }, ToggleInspector, None),
        KeyBinding::new(if is_mac { "cmd-t" } else { "ctrl-t" }, CycleTheme, None),
        KeyBinding::new(if is_mac { "cmd-d" } else { "ctrl-d" }, CompareSnapshots, None),
        KeyBinding::new(if is_mac { "cmd-1" } else { "ctrl-1" }, ViewSnapshots, None),
        KeyBinding::new(if is_mac { "cmd-2" } else { "ctrl-2" }, ViewAnalytics, None),
        KeyBinding::new(if is_mac { "cmd-3" } else { "ctrl-3" }, ViewSettings, None),
    ]);
}
