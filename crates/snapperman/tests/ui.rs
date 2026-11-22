use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Size, TestAppContext, WindowBounds,
    WindowOptions, px,
};
use snapper_core::backend::{
    AuthenticationRequiredError, MockSnapperBackend, SnapperBackend,
};
use snapper_core::model::{
    BatchDeleteReport, CleanupAlgorithm, ConfigInfo, ConfigSetting, CreateSnapshotRequest,
    DiffOutput, FileChange, ModifySnapshotRequest, Snapshot, SnapshotKey,
};
use snapperman::actions;
use snapperman::state::ActiveView;
use snapperman::theme::{self, AppThemePreset, CYBERPUNK_DRACULA_NAME};
use snapperman::{AppPrefs, SnapperShell, WorkspaceModel};

struct Harness {
    window: AnyWindowHandle,
    shell: Entity<SnapperShell>,
    workspace: Entity<WorkspaceModel>,
}

async fn boot_app(cx: &mut TestAppContext) -> Harness {
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::init_themes(cx, AppThemePreset::CyberpunkDracula);
        actions::bind_keys(cx);
    });

    let backend = Arc::new(MockSnapperBackend::seeded());
    let workspace = cx.new(|cx| WorkspaceModel::new(backend, cx));
    workspace.update(cx, |ws, cx| ws.refresh(cx));

    let shell_slot: Entity<Option<Entity<SnapperShell>>> = cx.new(|_| None);
    let slot_clone = shell_slot.clone();
    let ws_for_win = workspace.clone();

    let window = cx.update(|cx| {
        let bounds = Bounds::centered(None, Size::new(px(1280.), px(840.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        cx.open_window(options, move |window, cx| {
            let shell = cx.new(|cx| {
                SnapperShell::new_with_persistence(
                    ws_for_win.clone(),
                    AppPrefs::default(),
                    false,
                    window,
                    cx,
                )
            });
            slot_clone.update(cx, |s, _| *s = Some(shell.clone()));
            cx.new(|cx| Root::new(shell, window, cx))
        })
        .expect("open snapperman test window")
    });

    let handle: AnyWindowHandle = window.into();
    let shell = cx.read(|cx| {
        shell_slot
            .read(cx)
            .clone()
            .expect("SnapperShell should be initialized")
    });

    // Wait until async mock snapshots and used-space finish hydrating.
    cx.wait_for(handle, Duration::from_secs(3), {
        let ws = workspace.clone();
        move |window, cx| {
            let ws_ref = ws.read(cx);
            !ws_ref.loading_snapshots()
                && !ws_ref.snapshots().is_empty()
                && window.try_find("snapshot-row-root-145").is_some()
        }
    })
    .await;

    Harness {
        window: handle,
        shell,
        workspace,
    }
}

#[gpui_kit::test]
async fn default_cyberpunk_dracula_theme_and_mock_snapshots_load(cx: &mut TestAppContext) {
    let app = boot_app(cx).await;

    app.window
        .update(cx, |_, window, cx| {
            assert_eq!(Theme::global(cx).theme_name(), CYBERPUNK_DRACULA_NAME);
            let ws = app.workspace.read(cx);
            assert_eq!(ws.configs().len(), 3);
            assert_eq!(ws.snapshots().len(), 18);
            assert!(window.try_find("sidebar-config-root").is_some());
            assert!(window.try_find("snapshot-row-root-145").is_some());
        })
        .unwrap();
}

#[gpui_kit::test]
async fn filters_by_sidebar_config_and_search_query(cx: &mut TestAppContext) {
    let app = boot_app(cx).await;

    // Click 'home' config in the sidebar.
    app.window
        .update(cx, |_, window, cx| {
            window.click("sidebar-config-home", cx);
        })
        .unwrap();

    cx.wait_for(app.window, Duration::from_secs(2), {
        let ws = app.workspace.clone();
        move |window, cx| {
            ws.read(cx).filter().config.as_deref() == Some("home")
                && window.try_find("snapshot-row-home-88").is_some()
                && window.try_find("snapshot-row-root-145").is_none()
        }
    })
    .await;

    // Switch back to All Configs and type search query into toolbar search input.
    app.window
        .update(cx, |_, window, cx| {
            window.click("sidebar-config-all", cx);
            let toolbar = app.shell.read(cx).toolbar().clone();
            let input = toolbar.read(cx).search_input().clone();
            input.update(cx, |state, cx| {
                state.set_value("Hyprland", window, cx);
            });
            app.workspace
                .update(cx, |ws, cx| ws.set_query("Hyprland".to_string(), cx));
            window.render_frame(cx);
        })
        .unwrap();

    app.window
        .update(cx, |_, window, cx| {
            assert_eq!(app.workspace.read(cx).filtered_indices().len(), 1);
            assert!(window.try_find("snapshot-row-home-86").is_some());
        })
        .unwrap();
}

#[gpui_kit::test]
async fn opens_changes_tab_loads_diff_and_toggles_split_mode(cx: &mut TestAppContext) {
    let app = boot_app(cx).await;

    // Select snapshot #145 (has rich mock status changes) and switch to Changes tab.
    app.window
        .update(cx, |_, window, cx| {
            window.click("snapshot-row-root-145", cx);
            window.click("inspector-tab-changes", cx);
        })
        .unwrap();

    cx.wait_for(app.window, Duration::from_secs(3), {
        let ws = app.workspace.clone();
        move |window, cx| {
            let ws_ref = ws.read(cx);
            !ws_ref.status_loading()
                && !ws_ref.diff_loading()
                && !ws_ref.file_changes().is_empty()
                && ws_ref.diff_output().is_some()
                && window.try_find("diff-mode-split").is_some()
        }
    })
    .await;

    // Switch diff viewer to Split mode and verify buttons remain responsive.
    app.window
        .update(cx, |_, window, cx| {
            window.click("diff-mode-split", cx);
            assert!(window.try_find("diff-viewer-host").is_some());
        })
        .unwrap();
}

#[gpui_kit::test]
async fn creates_snapshot_through_create_dialog(cx: &mut TestAppContext) {
    let app = boot_app(cx).await;
    let initial_count = cx.read(|cx| app.workspace.read(cx).snapshots().len());

    app.window
        .update(cx, |_, window, cx| {
            window.click("toolbar-new-snapshot", cx);
        })
        .unwrap();

    cx.wait_for(app.window, Duration::from_secs(2), |window, _| {
        window.try_find("dialog-create-submit").is_some()
    })
    .await;

    app.window
        .update(cx, |_, window, cx| {
            window.input("UI integration test checkpoint", cx);
            window.click("dialog-create-submit", cx);
        })
        .unwrap();

    cx.wait_for(app.window, Duration::from_secs(3), {
        let ws = app.workspace.clone();
        move |_, cx| ws.read(cx).snapshots().len() == initial_count + 1
    })
    .await;
}

#[gpui_kit::test]
async fn guards_protected_snapshot_deletion(cx: &mut TestAppContext) {
    let app = boot_app(cx).await;
    let initial_count = cx.read(|cx| app.workspace.read(cx).snapshots().len());

    // Filter to var-log so snapshot #0 is among the 3 visible rows and click it.
    app.window
        .update(cx, |_, window, cx| {
            window.click("sidebar-config-var-log", cx);
        })
        .unwrap();

    cx.wait_for(app.window, Duration::from_secs(2), |window, _| {
        window.try_find("snapshot-row-var-log-0").is_some()
    })
    .await;

    app.window
        .update(cx, |_, window, cx| {
            window.click("snapshot-row-var-log-0", cx);
            window.render_frame(cx);
        })
        .unwrap();

    // Clicking disabled Delete on protected #0 must not remove any snapshot.
    app.window
        .update(cx, |_, window, cx| {
            assert!(window.try_find("inspector-protected-note").is_some());
            window.click("inspector-delete", cx);
            window.render_frame(cx);
            assert_eq!(app.workspace.read(cx).snapshots().len(), initial_count);
        })
        .unwrap();
}

#[gpui_kit::test]
async fn cycles_themes_and_navigates_analytics_and_settings(cx: &mut TestAppContext) {
    let app = boot_app(cx).await;

    app.window
        .update(cx, |_, window, cx| {
            assert_eq!(app.shell.read(cx).theme_preset(), AppThemePreset::CyberpunkDracula);
            window.click("titlebar-cycle-theme", cx);
            assert_eq!(app.shell.read(cx).theme_preset(), AppThemePreset::SystemDark);
            assert_eq!(Theme::global(cx).theme_name(), "Default Dark");

            window.click("titlebar-cycle-theme", cx);
            assert_eq!(app.shell.read(cx).theme_preset(), AppThemePreset::SystemLight);
            assert_eq!(Theme::global(cx).theme_name(), "Default Light");

            window.click("titlebar-cycle-theme", cx);
            assert_eq!(app.shell.read(cx).theme_preset(), AppThemePreset::CyberpunkDracula);
            assert_eq!(Theme::global(cx).theme_name(), CYBERPUNK_DRACULA_NAME);

            window.click("sidebar-nav-analytics", cx);
            window.render_frame(cx);
            assert_eq!(app.workspace.read(cx).active_view(), ActiveView::Analytics);
            assert!(window.try_find("analytics-view").is_some());

            window.click("sidebar-nav-settings", cx);
        })
        .unwrap();

    cx.wait_for(app.window, Duration::from_secs(3), {
        let ws = app.workspace.clone();
        move |window, cx| {
            ws.read(cx).active_view() == ActiveView::Settings
                && !ws.read(cx).settings_loading()
                && !ws.read(cx).config_settings().is_empty()
                && window.try_find("settings-view").is_some()
        }
    })
    .await;
}

struct AuthGatedMockBackend {
    inner: MockSnapperBackend,
    unlocked: AtomicBool,
}

impl AuthGatedMockBackend {
    fn new() -> Self {
        Self {
            inner: MockSnapperBackend::seeded(),
            unlocked: AtomicBool::new(false),
        }
    }

    fn require_auth(&self) -> Result<()> {
        if self.unlocked.load(Ordering::Relaxed) {
            Ok(())
        } else {
            Err(AuthenticationRequiredError::new(
                "Administrator privileges are required to access Snapper snapshots.",
            )
            .into())
        }
    }
}

impl SnapperBackend for AuthGatedMockBackend {
    fn backend_label(&self) -> String {
        if self.is_authenticated() {
            "Mock (sudo session)".to_string()
        } else {
            "Mock (Auth Required)".to_string()
        }
    }

    fn is_mock(&self) -> bool {
        true
    }

    fn supports_password_auth(&self) -> bool {
        true
    }

    fn is_authenticated(&self) -> bool {
        self.unlocked.load(Ordering::Relaxed)
    }

    fn authenticate_with_password(&self, password: &str) -> Result<()> {
        if password == "correct-pw" {
            self.unlocked.store(true, Ordering::Relaxed);
            Ok(())
        } else {
            anyhow::bail!("Incorrect password. Please try again.");
        }
    }

    fn clear_authentication(&self) {
        self.unlocked.store(false, Ordering::Relaxed);
    }

    fn list_configs(&self) -> Result<Vec<ConfigInfo>> {
        self.inner.list_configs()
    }

    fn list_snapshots(
        &self,
        config: Option<&str>,
        include_used_space: bool,
    ) -> Result<Vec<Snapshot>> {
        self.require_auth()?;
        self.inner.list_snapshots(config, include_used_space)
    }

    fn create_snapshot(&self, req: &CreateSnapshotRequest) -> Result<Option<u32>> {
        self.require_auth()?;
        self.inner.create_snapshot(req)
    }

    fn modify_snapshot(&self, req: &ModifySnapshotRequest) -> Result<()> {
        self.require_auth()?;
        self.inner.modify_snapshot(req)
    }

    fn delete_snapshots(&self, targets: &[SnapshotKey]) -> BatchDeleteReport {
        if let Err(err) = self.require_auth() {
            return BatchDeleteReport {
                deleted: Vec::new(),
                failed: targets.iter().map(|k| (k.clone(), err.to_string())).collect(),
            };
        }
        self.inner.delete_snapshots(targets)
    }

    fn rollback_snapshot(&self, key: &SnapshotKey) -> Result<()> {
        self.require_auth()?;
        self.inner.rollback_snapshot(key)
    }

    fn snapshot_status(&self, config: &str, from: u32, to: u32) -> Result<Vec<FileChange>> {
        self.require_auth()?;
        self.inner.snapshot_status(config, from, to)
    }

    fn snapshot_diff(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
        max_bytes: usize,
    ) -> Result<DiffOutput> {
        self.require_auth()?;
        self.inner
            .snapshot_diff(config, from, to, files, max_bytes)
    }

    fn undo_changes(
        &self,
        config: &str,
        from: u32,
        to: u32,
        files: &[String],
    ) -> Result<String> {
        self.require_auth()?;
        self.inner.undo_changes(config, from, to, files)
    }

    fn get_config(&self, config: &str) -> Result<Vec<ConfigSetting>> {
        self.require_auth()?;
        self.inner.get_config(config)
    }

    fn set_config(&self, config: &str, settings: &[ConfigSetting]) -> Result<()> {
        self.require_auth()?;
        self.inner.set_config(config, settings)
    }

    fn run_cleanup(&self, config: &str, algorithm: CleanupAlgorithm) -> Result<()> {
        self.require_auth()?;
        self.inner.run_cleanup(config, algorithm)
    }
}

#[gpui_kit::test]
async fn prompts_auth_dialog_when_backend_requires_authentication_and_loads_after_unlock(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::init_themes(cx, AppThemePreset::CyberpunkDracula);
        actions::bind_keys(cx);
    });

    let backend = Arc::new(AuthGatedMockBackend::new());
    let workspace = cx.new(|cx| WorkspaceModel::new(backend, cx));
    let ws_for_win = workspace.clone();

    let window: AnyWindowHandle = cx
        .update(|cx| {
            let bounds = Bounds::centered(None, Size::new(px(1280.), px(840.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };
            cx.open_window(options, move |window, cx| {
                let shell = cx.new(|cx| {
                    SnapperShell::new_with_persistence(
                        ws_for_win.clone(),
                        AppPrefs::default(),
                        false,
                        window,
                        cx,
                    )
                });
                cx.new(|cx| Root::new(shell, window, cx))
            })
            .expect("open gated snapperman test window")
        })
        .into();

    workspace.update(cx, |ws, cx| ws.refresh(cx));

    cx.wait_for(window, Duration::from_secs(3), {
        let ws = workspace.clone();
        move |window, cx| {
            let ws_ref = ws.read(cx);
            ws_ref.auth_required()
                && ws_ref.snapshots().is_empty()
                && window.try_find("dialog-auth-password").is_some()
                && window.try_find("snapshot-table-auth-state").is_some()
        }
    })
    .await;

    // Wrong password shows inline error, keeps dialog open, and keeps table empty.
    window
        .update(cx, |_, window, cx| {
            window.input("wrong-pw", cx);
            window.click("dialog-auth-submit", cx);
        })
        .unwrap();

    cx.wait_for(window, Duration::from_secs(2), {
        let ws = workspace.clone();
        move |window, cx| {
            window.try_find("dialog-auth-error").is_some()
                && window.try_find("dialog-auth-password").is_some()
                && ws.read(cx).snapshots().is_empty()
        }
    })
    .await;

    // Correct password closes dialog, unlocks session, and loads snapshots.
    window
        .update(cx, |_, window, cx| {
            window.input("correct-pw", cx);
            window.click("dialog-auth-submit", cx);
        })
        .unwrap();

    cx.wait_for(window, Duration::from_secs(3), {
        let ws = workspace.clone();
        move |window, cx| {
            let ws_ref = ws.read(cx);
            !ws_ref.auth_required()
                && ws_ref.is_authenticated()
                && window.try_find("dialog-auth-password").is_none()
                && window.try_find("snapshot-row-root-145").is_some()
        }
    })
    .await;
}
