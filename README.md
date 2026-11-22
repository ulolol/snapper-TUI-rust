# snapperman & snapper‑TUI‑rust 🚀

![Snapper TUI Rust Logo](snapper-tui-rust-logo.gif)

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/built_with-Rust-d65d0e.svg)](https://www.rust-lang.org/)

A Rust workspace for managing **Snapper** Btrfs/LVM snapshots with two native frontends powered by a shared core engine:

- **`snapperman`** — GPU‑accelerated desktop GUI built on **GPUI** and **`gpui-kit`**, featuring a **Soft Organic Aetherpunk / Arcanepunk Theme Suite** (**Aetherpunk Citadel** default, **Arcanepunk Enclave**, **Aether Parchment** light), floating rounded inset workspace panels, pill controls, preserved **Cyberpunk Dracula**, and **System Dark / Light** presets.
- **`snapper-TUI-rust`** — Fast, lightweight terminal user interface built on **Ratatui** with zero GUI dependencies.
- **`snapper-core`** — Shared backend (`org.opensuse.Snapper` D‑Bus + CLI/`pkexec` + deterministic `--mock`), typed models, tolerant parsers, filtering/sorting, file trees, and analytics.

![Demo](snapper-TUI-rust.gif)

---

## ✨ Features

### Desktop GUI (`snapperman`)
- **Soft Organic Aetherpunk / Arcanepunk Single-Pane UI & Theme Backdrops** — Seamless, borderless frosted workspace (`Sidebar`, `Toolbar`, `DataTable`, `Inspector`, `Analytics`, `Settings`) with transparent section/row/table borders and edge-to-edge layout flow layered over **6 bespoke, embedded abstract sculptural landscape backgrounds** (one matched per theme, switching live with `Cmd/Ctrl+T` or the Command Palette). Low-opacity frosted surfaces and atmospheric scrims keep backgrounds visible across both light (**Aether Parchment**, **System Light**) and dark (**Aetherpunk Citadel**, **Arcanepunk Enclave**, **Cyberpunk Dracula**, **System Dark**) presets while preserving crisp legibility. Features concentric `10px / 16px / pill` geometry, borderless pill resize handles, circular orbital astrolabe crest & S-wave filigree dividers, and **100% themed vector icons** via `gpui-kit`'s full `AllAssets` Lucide catalog (zero emojis in UI chrome).
- **🗂️ Floating Multi‑Config Vault Sidebar & Virtualized Table** — Tonal vault summary pill, `All Configs` or per‑config filtering (`root`, `home`, `var-log`, etc.) with snapshot count badges, two‑phase `used-space` hydration, pill-rounded storage micro-bars, multi‑select checkboxes with floating pill batch actions, sortable columns, actionable empty states with one-click filter reset, and row context menus.
- **🔬 Floating Specimen Inspector, File‑Change Tree & Native Diff Viewer** — Pill tabs (`Overview` / `Changes`), glowing circular status orb, borderless 2×2 tonal telemetry tiles (`Used Space`, `Retention Policy`, `Type / Pair Jump`, `File Delta`), borderless `DescriptionList`, risk-separated pill actions, blast-radius (`+A / ~M / -D / Attr`) diff summary pills, Unified & Split modes inside a sunken pill toggle track, `syntect` syntax highlighting, binary detection, 2 MiB truncation protection, arbitrary `A..B` / `N..0` comparison picker, and selective path restoration (`undochange`).
- **🛠️ Full Snapshot Lifecycle** — Create (`single`, `pre`, paired `post` snapshots, cleanup policy, `important=yes`, userdata), modify metadata/cleanup/pin state, delete with protected `#0`/`active`/`default` guards, and roll back with confirmation alerts across soft pill-actioned dialogs.
- **📊 Storage Analytics & Config Manager** — Soft floating KPI cards with circular icon medallions, per‑snapshot and per‑config `BarChart` visualizations, storage share bars with click‑to‑inspect navigation, and grouped `get-config` / `set-config` retention editing with human-readable key descriptions + staged **Discard / Save** banner and pill cleanup execution (`number`, `timeline`, `empty-pre-post`).
- **⌨️ Command Palette** — Instant `Cmd/Ctrl+K` fuzzy command palette for actions, views, configs, and themes.
- **🐧 Native Linux D‑Bus + Session Auth & CLI Fallback** — Connects directly to `org.opensuse.Snapper` via `zbus` on Linux, tries passwordless `sudo -n`, and presents a built‑in **Administrator Authentication** dialog (`sudo -S` single-prompt session with zeroed memory + non-hanging `pkexec --disable-internal-agent` option) so it works even on minimal window managers without a running Polkit agent.

### Terminal UI (`snapper-TUI-rust`)
- **🖥️ Cyberpunk TUI** — Lightweight terminal dashboard built with `ratatui`, `crossterm`, and `tachyonfx`.
- **🔄 Multi‑Config & Accurate Filtering** — Displays the `Cfg` column, supports sorting (`0`–`5` or clickable headers), search (`/`), batch multi‑select (`Space`), protected `#0`/`active`/`default` delete warnings, and safe compare‑range resolution when snapshots have gaps.
- **🧪 Cross‑Platform Mock Mode** — Run `snapper-TUI-rust --mock` on any machine (Linux or macOS) without modifying real snapshots.

---

## 🏗️ Workspace Structure

```text
snapper-tui-rust/
├── Cargo.toml                     # Workspace root (shared versions & profile optimization)
├── crates/
│   ├── snapper-core/              # Shared library: models, parsers, queries, CLI/D-Bus/Mock backends
│   ├── snapper-tui/               # Binary: `snapper-TUI-rust` (Ratatui TUI, no GPUI deps)
│   └── snapperman/                # Binary: `snapperman` (GPUI + gpui-kit desktop GUI)
│       ├── assets/
│       │   ├── backgrounds/       # Embedded 1600x900 WebP abstract landscape backdrops (6 presets)
│       │   └── themes/            # Embedded `aetherpunk-suite.json` & `cyberpunk-dracula.json` ThemeSets
│       └── tests/ui.rs            # Headless GPUI UI integration tests (`#[gpui_kit::test]`)
└── .github/workflows/rust.yml     # CI tests, Clippy, and dual-binary release builds
```

---

## 🛠️ Prerequisites & System Dependencies

- **Rust toolchain**: `rustc` / `cargo` **1.94+** (modern stable recommended via [`rustup`](https://rustup.rs/) for `gpui-kit` / `gpui`).
- **Snapper** (for live Linux usage): `snapper` installed and at least one config created (`snapper list-configs`).
- **Desktop GPU / Windowing libraries** (Linux build requirements for `snapperman`):

#### Arch / Garuda Linux
```bash
sudo pacman -S --needed pkgconf openssl zstd fontconfig vulkan-icd-loader libxcb libxkbcommon libxkbcommon-x11 wayland
```

#### Ubuntu / Debian
```bash
sudo apt-get update && sudo apt-get install -y \
  pkg-config libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libfontconfig1-dev libvulkan-dev libssl-dev libzstd-dev
```

#### Fedora
```bash
sudo dnf install -y \
  pkg-config libxcb-devel libxkbcommon-devel libxkbcommon-x11-devel \
  wayland-devel fontconfig-devel vulkan-loader-devel openssl-devel libzstd-devel
```

#### macOS (GUI Development & `--mock` Mode)
macOS uses Apple Metal / AppKit natively — no X11/Wayland packages are needed:
```bash
cargo run -p snapperman -- --mock
```

---

## 📦 Installation & Building

Build `snapper-TUI-rust` and `snapperman` separately so Cargo does not unify workspace features (`snapperman` enables `snapper-core/dbus` on Linux, whereas building `snapper-tui` on its own keeps the TUI binary free of GUI and D-Bus dependencies):

```bash
# 1️⃣ Clone the repository
git clone https://github.com/ulolol/snapper-TUI-rust.git
cd snapper-TUI-rust

# 2️⃣ Build the lightweight TUI binary (no GUI or D-Bus dependencies compiled)
cargo build -p snapper-tui --release

# 3️⃣ Build the GPUI desktop GUI binary separately
cargo build -p snapperman --release
```

Resulting binaries in `target/release/`:
- `target/release/snapper-TUI-rust` (Terminal UI)
- `target/release/snapperman` (GPUI Desktop GUI)

---

## 🚀 Usage

### `snapperman` (GUI)

```bash
# Linux: auto-selects D-Bus (org.opensuse.Snapper) with CLI/pkexec fallback
# macOS / non-Linux: auto-selects deterministic --mock mode
snapperman

# Force deterministic mock mode on any OS (safe testing/demos)
snapperman --mock

# Force Linux snapper CLI backend (with pkexec fallback)
snapperman --cli

# Force Linux D-Bus backend (falls back cleanly to CLI if unavailable)
snapperman --dbus
```

### `snapper-TUI-rust` (TUI)

```bash
# Live Snapper (uses `sudo -n snapper` when non-root and ACL access isn't configured)
sudo ./target/release/snapper-TUI-rust

# Mock mode (safe on macOS or Linux without root)
snapper-TUI-rust --mock
```

---

## 🔐 Privilege & Security Setup (Linux)

We recommend **unprivileged GUI execution** rather than running the full GUI process as `root`:

1. **Built‑in Single‑Prompt Session Auth (Default for `snapperman`)**: `snapperman` runs as your regular desktop user, probes `org.opensuse.Snapper` D‑Bus / direct `snapper` / `sudo -n`, and if elevation is needed opens a masked **Administrator Authentication** dialog (`sudo -S -k -p ""`). Credentials remain in memory only for that session (zeroed on **Lock** or exit), avoiding repeated Polkit prompts and working seamlessly in minimal WMs (Openbox, Hyprland, Sway, i3) even without a Polkit GUI agent. A secondary **Use Polkit (`pkexec`)** button (`--disable-internal-agent`) is also available when a Polkit agent is active.
2. **Optional Non‑Root Read Access (`ALLOW_USERS` + `SYNC_ACL`)**: To browse snapshots and configs without password prompts for your user:
   ```bash
   sudo snapper -c root set-config ALLOW_USERS="$USER" SYNC_ACL="yes"
   sudo snapper -c home set-config ALLOW_USERS="$USER" SYNC_ACL="yes"
   ```
3. **`snapper-TUI-rust` Sudo Usage**: Because `sudo` prompting inside raw terminal mode corrupts TUI drawing, run `sudo snapper-TUI-rust`, configure `ALLOW_USERS`/`SYNC_ACL`, or allow passwordless `sudo` for `/usr/bin/snapper`.

---

## ⌨️ Keyboard Shortcuts

### `snapperman` (GUI)

| Shortcut | Action |
|:---|:---|
| `Cmd/Ctrl + K` | Open Command Palette |
| `Cmd/Ctrl + N` | New Snapshot… |
| `Cmd/Ctrl + D` | Compare Snapshots… |
| `Cmd/Ctrl + F` | Focus Search Filter |
| `Cmd/Ctrl + R` / `F5` | Refresh Snapshots & Configs |
| `Cmd/Ctrl + B` | Toggle Sidebar |
| `Cmd/Ctrl + I` | Toggle Inspector |
| `Cmd/Ctrl + T` | Cycle Theme (Citadel → Enclave → Parchment → Cyberpunk → Dark → Light) |
| `Cmd/Ctrl + 1` / `2` / `3` | Switch View (Snapshots / Analytics / Config & Retention) |

### `snapper-TUI-rust` (TUI)

| Key | Action |
|:---|:---|
| `q` / `Q` | Quit application |
| `c` / `C` | **Create** a new snapshot |
| `d` / `D` | **Delete** selected snapshot(s) |
| `a` / `A` | **Apply** (rollback) to selected snapshot |
| `r` / `R` | **Refresh** snapshot list |
| `s` / `S` | Refresh **Status** of selected snapshot |
| `/` | **Filter** snapshots |
| `Space` | **Toggle Multi‑Selection** |
| `↑` / `↓` / `j` / `k` | Navigate snapshots (`J`/`K` or mouse scroll details pane) |
| `0`‑`5` | Sort by Config, Number, Type, Date, User, or Space |
| `Esc` | Cancel popup / Clear filter |

---

## 🧪 Testing

Run TUI and GUI checks separately to match CI and avoid workspace feature unification:

```bash
# TUI & Core clippy + tests (zero GUI / D-Bus dependencies)
cargo clippy -p snapper-core -p snapper-tui --all-targets -- -D warnings
cargo test -p snapper-core -p snapper-tui

# GUI & Core (with D-Bus) clippy + tests (includes headless GPUI UI tests)
cargo clippy -p snapper-core --features dbus --all-targets -- -D warnings
cargo clippy -p snapperman --all-targets -- -D warnings
cargo test -p snapper-core --features dbus
cargo test -p snapperman

# Linux read-only live probe against real snapper daemon/CLI (non-destructive)
cargo test -p snapper-core --features dbus --test live_readonly -- --ignored --nocapture
```

---

## 🤝 Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

1. Fork the repo.
2. Create a feature branch (`git checkout -b feat/awesome-feature`).
3. Ensure `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass.
4. Open a PR describing your changes.

## 📄 License

This project is licensed under the [MIT License](LICENSE).

---
© 2025 Vidish
