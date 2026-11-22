# AI Development Log (`AILog.md`)

## 2026-11-22 — `snapperman.gif` Showcase & `README.md` Update

### Steps & Verification
- **Stitched `snapperman.gif`**: Combined 11 captured `snapperman` screenshots from `~/Downloads/Screenshot_2026-11-22-*.png` using Python 3 + Pillow `13.0.0` (`ImageOps.fit` Lanczos to `1280×720`, per-frame 256-color MedianCut quantization with Floyd-Steinberg dithering, `duration=2300ms`, `disposal=2`, `loop=0`) into `/Users/kaos/Projects/snapper-tui-rust/snapperman.gif` (`2,804,060 bytes` / `2.67 MiB`). Verified via `magick identify` (11 frames at `1280x720`, `delay=230`, `Iterations: 0`) and visually inspected extracted frames `0`, `2`, and `4`.
- **Updated `README.md`**: Added `snapperman.gif` directly above `snapper-TUI-rust.gif`, added a dedicated `About snapperman (Desktop GUI)` section (architecture, 6-theme backdrop table, 3 primary views, and measured Garuda Linux footprint), added the prebuilt GitHub Releases section (`v0.7.3`), and updated `Contributing` check commands to use separate GUI/TUI package checks.

## 2026-11-22 — Release `v0.7.3`: Workspace Version Bump, Full CI Parity Checks & Tag Push

### Steps & Verification (`task-736`)
- **Version Bump**: Updated `[workspace.package]` version from `0.5.0` to `0.7.3` in `Cargo.toml` (inherited by `snapper-core`, `snapper-tui`, and `snapperman`) and refreshed `Cargo.lock`.
- **Full Local Verification (CI Parity)**:
  - `cargo clippy -p snapper-core -p snapper-tui --all-targets -- -D warnings`: 0 warnings (`1.39s`).
  - `cargo test -p snapper-core -p snapper-tui`: `17 passed; 0 failed` (`13` `snapper-core` + `4` `snapper-tui`).
  - `cargo clippy -p snapper-core --features dbus --all-targets -- -D warnings`: 0 warnings (`0.71s`).
  - `cargo test -p snapper-core --features dbus`: `16 passed; 0 failed`.
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: 0 warnings (`2.45s`).
  - `cargo test -p snapperman`: `15 passed; 0 failed` (`5` unit + `10` `tests/ui.rs` integration tests).

## 2026-11-22 — `snapperman` Pass 6: Inline Sidebar Count Pills (`aether::count_pill`) & Bounds Test

### Findings
- **Root Cause of Clipped Sidebar Count Badges**: `gpui_kit::component::badge::Badge` (`gpui-component-0.10.0/src/badge.rs`) is an absolute notification overlay designed to wrap a child icon, painting with `top(-3px)` and negative `inset_e(-3px * count.len(), cx)`. When used childless inside `SidebarMenuItem::suffix` (`crates/snapperman/src/views/sidebar.rs`), its `0x0` wrapper sits flush against `SidebarMenuItem`'s `.overflow_x_hidden()` flex container (`gpui-component-0.10.0/src/sidebar/menu.rs`), pushing the right side of the badge outside the clip edge and shifting it off-center. The same childless `Badge` was also used in `crates/snapperman/src/views/analytics.rs`.

### Steps & Fixes
- **`crates/snapperman/src/views/aether.rs`**: Added `pub fn count_pill(id, count, max, active, cx) -> AnyElement` returning a normal-flow `.flex_shrink_0()` pill with `h(px(18.))`, `min_w(px(22.))`, `px_1p5()`, `rounded_full()`, mono `11px` font, `.test_support()`, and theme-adaptive contrast (no `absolute()` or negative edge offsets).
- **`crates/snapperman/src/views/sidebar.rs`**: Replaced `Badge::new().count(...)` suffixes on `"All Configs"` and each per-config item with `aether::count_pill(...)` (`sidebar-config-all-count`, `sidebar-config-<name>-count`) and added `.py_1().px_2()` item padding on `SidebarMenuItem`.
- **`crates/snapperman/src/views/analytics.rs`**: Replaced the childless `Badge::new().count(top_rows.len())` with `aether::count_pill("analytics-top-count", top_rows.len(), 999, false, cx)`.
- **`crates/snapperman/tests/ui.rs`**: Added `#[gpui_kit::test]` async regression test `sidebar_count_pills_fit_inside_expanded_rows` asserting each expanded config row's count pill has explicit size (`>= 22x16`) and sits strictly inside the row's bounds (`pill.right() <= row.right() - px(4.)`, `pill.top() >= row.top()`, `pill.bottom() <= row.bottom()`).

### Verification & Deploy
- **macOS (`task-691`)**:
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: 0 warnings (`0.89s`).
  - `cargo test -p snapperman`: `15 passed; 0 failed` (`5` unit tests + `10` `tests/ui.rs` tests including `sidebar_count_pills_fit_inside_expanded_rows`).
  - `cargo clippy -p snapper-core -p snapper-tui --all-targets -- -D warnings` (`0.19s`) & `cargo test -p snapper-core -p snapper-tui`: `17 passed; 0 failed` (`13` `snapper-core` + `4` `snapper-tui`).
  - `git status --short crates/snapper-core crates/snapper-tui`: clean (no modifications).
- **Linux (`kaos@ka0sboxnvme.local`, `task-695`)**:
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: 0 warnings (`0.67s`).
  - `cargo test -p snapperman`: `15 passed; 0 failed` (`5` unit tests + `10` `tests/ui.rs` tests, `3.38s`).
  - `cargo build -p snapperman --release`: built in `40.69s`, stripped and installed to `/home/kaos/.local/bin/snapperman` (`49M` ELF x86-64 PIE binary, `LDD_CLEAN`, `HELP_OK`).

## 2026-11-22 — `snapperman` Pass 4 & Pass 5: Translucent Single-Pane Flow, Border Removal, Lucide Icon Fix (`AllAssets`), and Linux Redeploy

### Findings
- **Root Cause of Missing Circular Medallion Icons**: `crates/snapperman/src/main.rs` registered `.with_assets(gpui_kit::assets::Assets)`, which only embeds ~95 icons from `default-icons.txt` and omits `Gauge`, `Layers`, `Flame`, `Clock`, `FileDiff`, `ShieldCheck`, `ShieldAlert`, `KeyRound`, `Lock`, `LockOpen`, `SlidersHorizontal`, `Compass`, `GitCompareArrows`, `Archive`, `Pin`, `PinOff`, `RotateCcw`, `Sparkles`, `Zap`, `Activity`, `FolderTree`, and `X`. Switching to `gpui_kit::assets::AllAssets` bundles the full 1,830-icon Lucide SVG catalog.
- **Opacity & Borderless Single-Pane Layout**: High-opacity panel/table fills (especially on light presets `Aether Parchment` and `System Light`) obscured the embedded 6-theme WebP backgrounds, while panel inset gaps and 1px section/row/button borders fragmented the canvas into visible boxes.

### Steps & Fixes
- **`crates/snapperman/src/main.rs`**: Registered `.with_assets(gpui_kit::assets::AllAssets)`.
- **`crates/snapperman/src/theme.rs`**: Lowered `ThemeAccents` glass opacities across dark/light presets (`backdrop_scrim`: `0.08`/`0.03`, `chrome_bg`: `0.36`/`0.30`, `surface_sidebar`: `0.40`/`0.34`, `surface_panel`: `0.46`/`0.40`, `surface_card`: `0.44`/`0.38`, `surface_diff`: `0.62`/`0.58`, `sunken_bg`: `0.24`/`0.22`, `header_tint`: `0.26`/`0.24`), configured translucent `table.*` tokens (`0.45`/`0.40` body, `0.50`/`0.47` head/foot, `0.09` stripe, `0.48`/`0.45` hover), aligned `SystemDark`/`SystemLight` radii to `10`/`16`, and set `border`, `sidebar_border`, `table_row_border`, `title_bar_border`, and `status_bar_border` to `transparent_black()`.
- **`crates/snapperman/assets/themes/aetherpunk-suite.json` & `cyberpunk-dracula.json`**: Updated radii (`radius: 10`, `radius.lg: 16`), set `border`, `sidebar.border`, `table.row.border`, `title_bar.border`, and `status_bar.border` to `#00000000`, softened `input.border`, and applied translucent `table.*` (`#0E192A73`, `#130F2173`, `#FAF6EE66`, `#15162273`) and `button.secondary.*` fills.
- **`crates/snapperman/src/views/aether.rs` & Views (`shell`, `sidebar`, `toolbar`, `table`, `inspector`, `diff_pane`, `analytics`, `settings`, `command_palette`, `dialogs/{mod,auth,create,modify,compare,confirm}`)**:
  - Removed inset margins/gaps between Sidebar, Toolbar, Table, and Inspector and removed `.border_1()`/`.border_t_1()`/shadows across panels, cards, medallions, Settings rows, dirty banner, and batch bar.
  - Switched `.outline()` Tags to `.border_0()`, `.outline()` secondary buttons to `.secondary()`, and added frosted `analytics-header-card` and `settings-header-card`.
  - Added `all_snapperman_lucide_icons_load_from_all_assets` unit test validating all 35 `LucideIconName` variants against `AllAssets`.

### Verification & Deploy
- **macOS**:
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: 0 warnings (`1.00s`).
  - `cargo test -p snapperman`: `14 passed; 0 failed` (5 unit tests + 9 `tests/ui.rs` integration tests).
  - `cargo clippy -p snapper-core -p snapper-tui --all-targets -- -D warnings`: 0 warnings (`0.18s`); `cargo test -p snapper-core -p snapper-tui`: `17 passed; 0 failed` (`13` `snapper-core` + `4` `snapper-tui`).
  - Python source scan on `crates/snapperman/src/**`: `ZERO EMOJIS VERIFIED`.
- **Linux (`kaos@ka0sboxnvme.local`)**:
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: 0 warnings (`0.69s`).
  - `cargo test -p snapperman`: `14 passed; 0 failed` (5 unit tests + 9 `tests/ui.rs` tests, `3.06s`).
  - `cargo build -p snapperman --release`: built in `38.23s`, stripped and installed to `/home/kaos/.local/bin/snapperman` (`49M` ELF x86-64 binary, `LDD_CLEAN`, `HELP_OK`).

## 2026-11-22 — `snapperman` Pass 3: Theme-Matched Abstract Sculptural Landscape Backgrounds

### Findings
- Added bespoke abstract solid-object / landscape artwork for each of the 6 `AppThemePreset` themes while preserving strict WCAG AA/AAA text & UI legibility through atmospheric scrims and frosted translucent panel/table surfaces.
- Verified `gpui-pre` enables `image` with `webp`, `jpeg`, and `png` decoders, and `gpui_kit::img(Arc<Image>)` with `ObjectFit::Cover` decodes embedded `ImageFormat::Webp` assets cleanly inside both runtime windows and headless `#[gpui_kit::test]` runners.
- Identified that `DataTable` paints `cx.theme().tokens.table`, so making `table.background` frosted while keeping `snapshot-table-container` transparent during `DataTable` rendering avoids double-stacking opacity; additionally, `ThemeTokens::reconcile` syncs `theme.colors.*` updates automatically onto `theme.tokens.*` for `SystemDark`/`SystemLight` presets.

### Steps
1. Generated 6 custom 16:9 minimalist 3D sculptural / landscape wallpapers via `image-generator` (matching Aetherpunk Citadel obsidian/brass/aether, Arcanepunk Enclave obsidian-violet/amethyst/emerald, Aether Parchment sunlit travertine/ivory/brass, Cyberpunk Dracula nocturnal indigo/violet/cyan, System Dark charcoal basalt dunes, and System Light high-key porcelain/alabaster), visually inspected all 6 with zero text/artifacts, and encoded them to compact `1600x900` `.webp` files in `crates/snapperman/assets/backgrounds/` (`12K`–`36K` each, ~`153 KB` combined).
2. Embedded all 6 WebP assets via `include_bytes!` and `OnceLock<Arc<Image>>` in `crates/snapperman/src/theme.rs` (`theme_background_image(preset)`), extended `ThemeAccents` with `backdrop_scrim`, `chrome_bg`, `surface_panel`, `surface_sidebar`, `surface_card`, and `surface_diff`, tuned translucent table tokens in `aetherpunk-suite.json` & `cyberpunk-dracula.json` (plus `10/16` radius in `cyberpunk-dracula.json`), and harmonized `SystemDark` / `SystemLight` radii, rim borders, and frosted tables inside `apply_theme_preset`.
3. Added `aether::theme_backdrop(preset, cx)` and wired it into `SnapperShell` behind frosted `TitleBar` / `StatusBar`, floating translucent `Sidebar`, `Toolbar`, `DataTable`, `Inspector`, `DiffPane`, `AnalyticsView` (with frosted `analytics-header-card`), and `SettingsView` (with frosted `settings-header-card`) so every theme switch updates the artwork cleanly while keeping every label, icon, chart, and diff line sharp and legible.
4. Added unit test `theme_background_images_are_valid_webp_and_distinct` validating RIFF/WEBP headers and unique hashes across all 6 presets.
5. Updated `README.md`, `AILog.md`, and `walkthrough.md`.

### Progress & Verification
- **Emoji/Pictograph Audit (`crates/snapperman/src/**`)**: **0 hits** (`ZERO EMOJIS VERIFIED`).
- **macOS (local, separate GUI & TUI invocations)**:
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: **0 warnings/errors**.
  - `cargo test -p snapperman`: **13 passed, 0 failed** (`4` unit + `9` `#[gpui_kit::test]` headless UI tests exercising all 6 theme switches).
  - `cargo clippy -p snapper-core -p snapper-tui --all-targets -- -D warnings` & `cargo test -p snapper-core -p snapper-tui`: **17 passed, 0 failed** (`13` `snapper-core` + `4` `snapper-tui`).
- **Garuda Linux (`kaos@ka0sboxnvme.local`)**:
  - Synced workspace to `~/snapperman-test/`.
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: **0 warnings/errors** (`0.78s`).
  - `cargo test -p snapperman`: **13 passed, 0 failed** (`4` unit + `9` `#[gpui_kit::test]` UI tests).
  - `cargo build -p snapperman --release` (`40.58s`): installed stripped x86-64 ELF binary (`48M`) to `/home/kaos/.local/bin/snapperman`; verified `ldd` clean (`LDD_CLEAN`) and `--help` exits cleanly (`HELP_OK`).

---

## 2026-11-22 — `snapperman` Pass 2: Soft Organic Aetherpunk & Floating Rounded Overhaul

### Findings
- User feedback on the Pass 1 Linux install identified the layout as overly boxy (caused by `radius: 3`, `radius.lg: 5`, opaque 1px borders across nested cards/grids/toolbars, angular 90° `corner_bracket_overlay`, octagonal/diamond `brand_sigil`, rigid top 2px bars on `plaque_card`, and edge-to-edge docked panes divided by 1px steel lines).
- Online & design-system research synthesized a **Soft Organic Aetherpunk / Celestial Art Nouveau** direction combining:
  - **Concentric radius math** (`16px` outer floating panels/cards → `10px` nested controls/inputs/tiles → `9999px` pill tags, filter chips, tabs, and action buttons).
  - **Tonal surface elevation & two-layer ambient shadows** over a deeper recessed outer canvas (`#070D17` Citadel / `#090711` Enclave / `#EAE2D2` Parchment) so floating panels read via lift and whisper-translucent (`8–14%` alpha) rims rather than opaque box walls.
  - **GPUI curved vector primitives** (`PathBuilder::arc_to` & `cubic_bezier_to`) plus `gpui-kit` pill variants (`TabBar::pill()`, `Tag::rounded_full()`, `DescriptionList::bordered(false)`, `ButtonRounded::Size(px(9999.))`, custom `ResizeHandleRenderer`).

### Steps
1. Updated `crates/snapperman/assets/themes/aetherpunk-suite.json` across **Aetherpunk Citadel**, **Arcanepunk Enclave**, and **Aether Parchment** to `radius: 10` / `radius.lg: 16`, deep recessed canvas grounds, three-tier elevated surfaces, whisper-translucent rim borders, and WCAG 1.4.11 ≥3:1 `input.border` tokens.
2. Extended `ThemeAccents` in `crates/snapperman/src/theme.rs` with `header_tint`, `orb_halo`, `subtle_border`, `shadow_primary`, and `shadow_ambient`, and updated theme unit test assertions for `10`/`16` radii.
3. Rebuilt `crates/snapperman/src/views/aether.rs`:
   - Removed `corner_bracket_overlay`, `plaque_card`, `filigree_rule`, and `empty_state_plaque`.
   - Added circular orbital astrolabe `brand_sigil` (`arc_to` rings + core orb), sinuous cubic-Bézier `filigree_wave` with centered pearl orb, `floating_panel`, `soft_card`, circular `icon_medallion`, `pill_radius()`, two-layer `panel_shadows`/`card_shadows`, transparent gutter `soft_resize_handle()`, `status_dot()`, pill `space_micro_bar`, pill `Skeleton` loaders, and `empty_state_card`.
4. Overhauled all `snapperman` views (`shell.rs`, `sidebar.rs`, `toolbar.rs`, `table.rs`, `inspector.rs`, `diff_pane.rs`, `analytics.rs`, `settings.rs`, and `dialogs/{mod,auth,create,modify,compare,confirm}.rs`) around inset `16px`-rounded floating panels, borderless tonal `telemetry_cell`s & `.bordered(false)` `DescriptionList`, `.pill()` Inspector tabs, full-pill filter/diff-mode tracks, and pill buttons while preserving all headless UI test `ElementId`s and keeping `0` emojis.
5. Updated `README.md`, `AILog.md`, and `walkthrough.md`.

### Issues & Fixes
- Fixed `PathBuilder::cubic_bezier_to` endpoint argument type (`point(x, y)` instead of scalar `Pixels`) and removed unused `v_flex` import in `toolbar.rs` during initial compile check.
- Wrapped remote SSH deploy command under `bash -c '...'` because `kaos@ka0sboxnvme.local`'s default login shell is `fish`, avoiding POSIX `if ... then ... fi` syntax failure.

### Progress & Verification
- **Emoji/Pictograph Audit (`crates/snapperman/src/**`)**: **0 hits** (`ZERO EMOJIS VERIFIED`).
- **macOS (local, separate GUI & TUI invocations)**:
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: **0 warnings/errors**.
  - `cargo test -p snapperman`: **12 passed, 0 failed** (`3` unit + `9` `#[gpui_kit::test]` UI tests).
  - `cargo clippy -p snapper-core -p snapper-tui --all-targets -- -D warnings` & `cargo test -p snapper-core -p snapper-tui`: **17 passed, 0 failed** (`13` `snapper-core` + `4` `snapper-tui`).
- **Garuda Linux (`kaos@ka0sboxnvme.local`)**:
  - Synced workspace to `~/snapperman-test/`.
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: **0 warnings/errors** (`0.77s`).
  - `cargo test -p snapperman`: **12 passed, 0 failed** (`3` unit + `9` `#[gpui_kit::test]` UI tests).
  - `cargo build -p snapperman --release` (`37.62s`): installed stripped x86-64 ELF binary (`48M`) to `/home/kaos/.local/bin/snapperman`; verified `ldd` clean and `--help` exits cleanly.

---

## 2026-11-22 — `snapperman` Aetherpunk & Arcanepunk UI/UX Redesign

### Findings
- Researched **Aetherpunk / Arcanepunk** aesthetics (Hextech / Piltover Art-Deco precision, Kaladesh / Avishkar, Eberron) and audited every `snapperman` view and theme token against `gpui-component 0.10.0`.
- Discovered 5 silently ignored JSON theme keys in `crates/snapperman/assets/themes/cyberpunk-dracula.json` (`description_list_label.*`, `drag_border`, `link.*`, `progress_bar.background`, `slider.bar.background`) that did not match `ThemeConfigColors` serde field names (`description_list.label.*`, `drag.border`, `link` / `link.active` / `link.hover`, `progress.bar.background`, `slider.background`).
- Verified all color pairings in the three new themes (**Aetherpunk Citadel**, **Arcanepunk Enclave**, **Aether Parchment**) meet WCAG 2.2 AA/AAA contrast (`15.4:1`, `16.5:1`, `14.1:1` primary ink-on-ground; `7.2:1`, `6.7:1`, `5.2:1` muted-on-surface).
- Identified UX gaps across `snapperman`: duplicate `"snapperman"` header chrome in `TitleBar` and `SidebarHeader`, missing keyboard shortcut hints (`Kbd`), flat toolbar filter/action hierarchy, lack of storage magnitude signals in table & analytics rows, missing discard control + key descriptions in `SettingsView`, unseparated destructive vs. safe actions in `InspectorView`, and plain-text loading/empty states.
- Verified at `960px` minimum window width with expanded `255px` sidebar that `ToolbarView` and `InspectorView` headers need `.flex_wrap()` to avoid squeezing or clipping controls.

### Steps
1. Created `crates/snapperman/assets/themes/aetherpunk-suite.json` defining **Aetherpunk Citadel** (new default), **Arcanepunk Enclave**, and **Aether Parchment** (`radius: 3`, `radius.lg: 5`, native `linear-gradient(...)` specular brass/amethyst/chrome tokens, and bespoke `highlight` diff/syntax palettes).
2. Fixed all 5 schema key names in `crates/snapperman/assets/themes/cyberpunk-dracula.json` and preserved it as a switchable preset.
3. Updated `crates/snapperman/src/theme.rs` & `crates/snapperman/src/state/prefs.rs` with 6-preset `AppThemePreset`, `ThemeAccents` helper (theme-adaptive, zero hardcoded hex in views), and unit tests validating gradient and repaired schema token application.
4. Built `crates/snapperman/src/views/aether.rs` with GPUI `canvas`/`PathBuilder` vector chrome (`brand_sigil`, `filigree_rule`, `corner_bracket_overlay`, `plaque_card`, `space_micro_bar`, `shortcut_kbd`, `Skeleton` loaders, `empty_state_plaque`).
5. Redesigned all `snapperman` views (`shell.rs`, `sidebar.rs`, `toolbar.rs`, `table.rs`, `inspector.rs`, `diff_pane.rs`, `analytics.rs`, `settings.rs`, `command_palette.rs`, and `dialogs/{mod,auth,create,modify,compare,confirm}.rs`) using themed `gpui_kit::assets::IconName` vector icons everywhere with zero emojis.
6. Updated `crates/snapperman/tests/ui.rs` for the new default theme, 6-theme cycling, zero-match filter recovery (`table-clear-filters`), and staged settings discard (`settings-discard`).
7. Updated `README.md`.

### Progress & Verification
- **Icon & Emoji Audit**: Verified `0` emoji/pictograph codepoints across `crates/snapperman/src/**` (all UI chrome, badges, buttons, tabs, empty states, and dialogs use themed `gpui_kit::assets::IconName` / `IconName` or GPUI vector `PathBuilder` paths).
- **macOS (local, separate TUI & GUI invocations)**:
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: **0 warnings/errors**.
  - `cargo test -p snapperman`: **12 passed, 0 failed** (`3` theme/preset unit tests + `9` `#[gpui_kit::test]` headless UI tests).
  - `cargo clippy -p snapper-core -p snapper-tui --all-targets -- -D warnings` & `cargo test -p snapper-core -p snapper-tui`: **17 passed, 0 failed** (`13` `snapper-core` + `4` `snapper-tui`).
  - `cargo run -p snapperman -- --help`: clean exit 0.
- **Garuda Linux (`kaos@ka0sboxnvme.local`)**:
  - `cargo clippy -p snapperman --all-targets -- -D warnings`: **0 warnings/errors**.
  - `cargo test -p snapperman`: **12 passed, 0 failed** (`3` unit + `9` `#[gpui_kit::test]` UI tests).
  - `cargo build -p snapperman --release` (`47.50s`): installed stripped x86-64 ELF binary (`48M`) to `/home/kaos/.local/bin/snapperman`; verified all shared libraries resolved via `ldd`; verified `/home/kaos/.local/bin/snapperman --help` exits cleanly; confirmed `/home/kaos/.local/share/applications/snapperman.desktop` present and no stale `prefs.json` forcing the old theme.

---

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
