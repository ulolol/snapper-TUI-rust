# AI Development Log (`AILog.md`)

## 2026-11-22 — Release v0.5.0 & Separate TUI/GUI Build Pipeline

### Findings
- Building the full workspace together (`cargo build --workspace --release`) causes Cargo feature unification to enable `snapper-core/dbus` (via `snapperman`'s Linux target dependency) during `snapper-tui` builds, pulling `zbus` into `snapper-TUI-rust` unnecessarily.
- Separating `snapper-tui` and `snapperman` into independent Cargo invocations and GitHub Actions jobs verifies that `snapper-TUI-rust` builds with zero GUI/X11/Wayland/D-Bus dependencies.

### Steps
1. Bumped `[workspace.package] version` in `Cargo.toml` to `0.5.0` (inherited by `snapper-core`, `snapper-tui`, and `snapperman`) and synced `Cargo.lock`.
2. Split `.github/workflows/rust.yml` into three jobs:
   - `tui` (`Build & Test TUI`): runs `cargo clippy`, `cargo test`, and `cargo build -p snapper-tui --release` without installing GUI system packages, uploading `snapper-TUI-rust`.
   - `gui` (`Build & Test GUI (snapperman)`): installs Linux GPUI system libraries (including `libx11-dev` and `libx11-xcb-dev`), runs `cargo clippy` and `cargo test` for `snapper-core --features dbus` and `snapperman`, builds `target/release/snapperman`, and uploads `snapperman`.
   - `release` (`Publish GitHub Release`): runs on `v*` tag pushes after `tui` and `gui` succeed, downloads both artifacts, restores `+x` permissions, and publishes `Release vX.Y.Z` with both binaries attached.
3. Updated `README.md` and `how-to-release.md` to document separate TUI and GUI build, test, and tag-release workflows.

### Progress & Verification
- **macOS (local, separate TUI & GUI invocations)**:
  - `cargo clippy -p snapper-core -p snapper-tui --all-targets -- -D warnings` & `cargo test -p snapper-core -p snapper-tui`: **17 passed, 0 failed** (`13` `snapper-core` + `4` `snapper-tui`).
  - `cargo build -p snapper-tui --release`: succeeded (`target/release/snapper-TUI-rust`, `1.2M`).
  - `cargo clippy -p snapper-core --features dbus --all-targets -- -D warnings` & `cargo test -p snapper-core --features dbus`: **16 passed, 0 failed**.
  - `cargo clippy -p snapperman --all-targets -- -D warnings` & `cargo test -p snapperman`: **8 passed, 0 failed** (`1` unit + `7` `#[gpui_kit::test]` UI tests).
  - `cargo build -p snapperman --release`: succeeded (`target/release/snapperman`, `24M`).

---

## 2026-11-21 — Workspace Refactor & GPUI Desktop App (`snapperman`)

### Findings
- Audited v0.1.9 codebase (`Cargo.toml`, `src/{main,app,data,ui}.rs`): identified domain/UI coupling in `App`, uncancelled `mpsc::Receiver` overwrites on rapid navigation, missing `-c <config>` in `create_snapshot()`, naive `number - 1` assumption in `get_snapshot_status()`, silent batch delete error dropping, synchronous `used-space` blocking on Btrfs, and Linux `/proc/self/status` coupling preventing macOS UI development/testing.
- Evaluated `gpui-kit = "0.10.0"` (`gpui-pre` + `gpui-base` + `gpui-component` + `gpui-kit-assets`): provides `DataTable`, `Diff` (Unified/Split + `syntect`), `Tree`, `Sidebar`, `Resizable`, `TitleBar`, `StatusBar`, `DescriptionList`, `Dialog`/`AlertDialog`, `Command` palette, `BarChart`, `Notification`, and `#[gpui_kit::test]` headless UI testing.
- Verified live `org.opensuse.Snapper` D-Bus signatures on Garuda Linux (`snapper 0.13.2`) via `busctl introspect`, and discovered via live unprivileged probing that `snapperd` allows `ListConfigs` while rejecting `ListSnapshots` with `error.no_permissions` unless configured with `ALLOW_USERS` or root — so `SnapperDbusBackend` now tracks `needs_elevation` and falls back cleanly to `SnapperCliBackend` (`PkexecFallback`) on permission errors.
- Discovered via live `#117..0` Btrfs comparison (`369,380` changed paths) that storing descendant leaf vectors per folder in `DiffPaneView` creates unnecessary memory overhead and expansion cost; refactored `NodeMeta` to store `leaf_count` instead of cloned path vectors, auto-expand only for `<= 250` changed paths (or active path search), and resolve subtree paths on demand.
- Identified `gpui-pre-util v0.3.12` requires Rust `> 1.92` (`slice::as_array` stabilization), updated CI and README prerequisites accordingly, and removed unused direct `crossterm` dependency from `crates/snapper-tui/Cargo.toml`.
- Diagnosed live Openbox failure on Garuda Linux where `~/.config/openbox/autostart` referenced an uninstalled `xfce-polkit` binary: without a running graphical Polkit agent, `pkexec snapper ...` spawned its internal TTY helper (`polkit-agent-helper-1`) blocking indefinitely on hidden `tty3` so no password dialog appeared and snapshots never loaded.

### Steps
1. Converted repository into a multi-crate Cargo workspace (`crates/snapper-core`, `crates/snapper-tui`, `crates/snapperman`).
2. Built `crates/snapper-core` with typed models (`SnapshotKey`, `CreateSnapshotRequest`, `ModifySnapshotRequest`, `FileChange`, `FileTreeNode`, `DiffOutput`, `BatchDeleteReport`), JSON & 6-char status parsers, collapsed file tree builder, zero-alloc ASCII filter/sort/range-resolution queries, and pluggable `SnapperBackend` (`SnapperDbusBackend` under `dbus` feature, `SnapperCliBackend` with `Direct`/`SudoNonInteractive`/`PkexecFallback`, plus seeded `MockSnapperBackend`).
3. Migrated `crates/snapper-tui` (`snapper-TUI-rust` binary) onto `snapper-core` with `--mock` support, `-h`/`--help` CLI handling, `Cfg` column (`0` key + header click), separate status channel, wrapped line scrolling, protected `#0`/`active`/`default` delete warnings, and zero GPUI dependencies.
4. Implemented `crates/snapperman` (`snapperman` GUI binary):
   - Embedded `Cyberpunk Dracula` default theme (`assets/themes/cyberpunk-dracula.json`) + instant switching to `System Dark` and `System Light` (`src/theme.rs`).
   - Reactive `WorkspaceModel` + persisted `AppPrefs` (`src/state/{workspace,prefs}.rs`) and OS-aware shortcuts (`src/actions.rs`).
   - Full GPUI views (`src/views/`): `TitleBar`, collapsible `Sidebar`, `Toolbar` with multi-select batch bar, virtualized `DataTable` with context menus, tabbed `InspectorView` (`Overview` + `Changes`), `DiffPaneView` (`Tree` + Unified/Split `Diff` + binary/2 MiB truncation handling + `undochange` restore), `Command` palette (`Cmd/Ctrl+K`), Create/Modify/Compare/Confirm/Cleanup dialogs, `AnalyticsView` (`BarChart`s), and `SettingsView` (`get-config`/`set-config`).
   - Added 6 headless `#[gpui_kit::test]` integration tests (`crates/snapperman/tests/ui.rs`).
5. Updated `.github/workflows/rust.yml` (Linux GPUI system dependencies, Clippy, workspace tests, dual release binaries) and `README.md`.

### Issues & Fixes
- Fixed `SnapperShell` missing `cx.observe(&workspace, ...)` so sidebar view navigation immediately re-renders the active view.
- Fixed `.overflow_y_scrollbar()` overriding `ElementId`s by placing test/domain IDs on inner view containers in `analytics.rs` and `settings.rs`.
- Fixed `TableDelegate::perform_sort` re-entrancy by deferring `workspace.set_sort(...)` out of the `TableState` borrow scope.
- Added `-h`/`--help` early exit to `crates/snapper-tui/src/main.rs` so `--help` never enters terminal raw mode.
- Added transparent `error.no_permissions` fallback from `SnapperDbusBackend` to `SnapperCliBackend` (`PkexecFallback`) and removed unused `crossterm` dependency from `crates/snapper-tui/Cargo.toml`.
- Switched `Cargo.toml` `[profile.release]` to `lto = "thin"` for fast multi-core release linking.
- Fixed silent `pkexec` hang & added single-prompt session authentication:
  - Always pass `--disable-internal-agent` and `Stdio::null()` to `pkexec` so missing Polkit GUI agents fail fast with a clear diagnostic instead of blocking on a hidden TTY.
  - Added typed `AuthenticationRequiredError`, `SecretPassword` (redacted `Debug`, zeroized via `.fill(0)` + `compiler_fence` on `Drop`), `sudo -n` fast check, `sudo -S -k -p ""` session execution, and `enable_pkexec_mode()` to `SnapperCliBackend` and forwarded via `SnapperDbusBackend`.
  - Ordered `main.rs` so `SnapperShell` subscribes before `ws.refresh(cx)`, added `open_auth_dialog` (`Auth` modal with masked `Input`, Enter submission, inline errors, and **Use Polkit (pkexec)** fallback), **Authenticate** / **Unlocked** + **Lock** controls in `TitleBar`, and interactive auth/retry empty state in `SnapshotTableView`.
  - Added `prompts_auth_dialog_when_backend_requires_authentication_and_loads_after_unlock` in `crates/snapperman/tests/ui.rs`.

### Progress & Verification
- **macOS (local)**:
  - `cargo test --workspace`: 13 (`snapper-core`) + 4 (`snapper-tui`) + 1 (`snapperman` unit) + 7 (`snapperman` UI tests) = **25 passed, 0 failed**.
  - `cargo test -p snapper-core --features dbus`: **16 passed, 0 failed**.
  - `cargo clippy --workspace --all-targets -- -D warnings` & `cargo clippy -p snapper-core --features dbus --all-targets -- -D warnings`: **0 warnings/errors**.
  - `snapperman --mock` GUI launch sanity check: passed cleanly.
- **Garuda Linux (`kaos@ka0sboxnvme.local`, Rust 1.100.0, `snapper 0.13.2`, X11/Openbox `DISPLAY=:0`)**:
  - `cargo test --workspace` (with `dbus` enabled): 16 (`snapper-core`) + 4 (`snapper-tui`) + 1 (`snapperman` unit) + 7 (`snapperman` UI tests) = **28 passed, 0 failed** (1 ignored `live_read_only_snapper_probe`).
  - `cargo clippy --workspace --all-targets -- -D warnings`: **0 warnings/errors**.
  - `cargo build --workspace --release` (`thin` LTO, `1m 43s`): installed stripped x86-64 ELF production binaries (`/home/kaos/.local/bin/snapperman` 48M, `/home/kaos/.local/bin/snapper-TUI-rust` 1.4M) and `~/.local/share/applications/snapperman.desktop`; verified zero missing dynamic libraries via `ldd`; verified `--help` exits cleanly; verified live non-mock `snapperman` startup on `DISPLAY=:0` stays alive cleanly with zero hung `pkexec` child processes.
  - Read-only live probe (`live_read_only_snapper_probe`) verified live `org.opensuse.Snapper` D-Bus and CLI against real `root` config (`7` snapshots, `23` settings, `369,380` changed paths in `#117..0`).
  - `snapperman --mock` launched and ran cleanly on the active X11 display (`DISPLAY=:0`).

---

## 2026-11-18 — Deep Code Review, Bug Fixes & Optimization

### Findings
- **Critical**: `App::get_selected_snapshot()` and `App::get_targets_for_delete()` indexed into unfiltered `self.snapshots` while `ui::draw_snapshot_table()` rendered `get_filtered_snapshots()`. `selected_indices: HashSet<usize>` also shifted targets when sorting or filtering.
- **Critical**: `data.rs` did not pass `-c <config>` to `delete`, `rollback`, or `status`, causing multi-config snapshots to target `root`.
- **High**: `sudo` was invoked interactively in raw mode; `ui::draw_header()` was called twice per frame; `tachyonfx` received cumulative elapsed time instead of frame delta; mouse click-to-select ignored `table_state.offset()`, modal/splash states, and centered footer button positions; `main.rs` skipped `sort_snapshots()` on async load.
- **Cleanup**: Unused dependencies (`clap`, `color-to-tui`, `ratatui-macros`), dead `InputMode` and `App::refresh_snapshots()`, and repeated per-frame `to_lowercase()`/`String` allocations.

### Steps & Fixes
1. Cleaned up `Cargo.toml` unused dependencies.
2. Updated `src/data.rs` with `-c <config>`, root check + `sudo -n` non-interactive execution, and `BTreeMap` for `userdata`.
3. Refactored `src/app.rs` to key multi-selections by `(String, u32)`, resolve selections against filtered snapshots, optimize filtering, bound scroll offsets, and add unit tests.
4. Updated `src/ui.rs` to remove duplicate `draw_header()`, fix `tachyonfx` delta timing, avoid row string clones, and record exact `UiAreas` rects.
5. Updated `src/main.rs` with `TerminalGuard` RAII cleanup, deduplicated async helpers, `status_request_id` stale-response protection, `KeyEventKind::Release` filtering, and `UiAreas`-based mouse hit-testing.

### Progress
- `cargo check --all-targets`, `cargo test` (4/4 passing), and `cargo clippy --all-targets -- -D warnings` all pass with zero warnings.
