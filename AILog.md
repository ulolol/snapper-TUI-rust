# AI Development Log (`AILog.md`)

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
