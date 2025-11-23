# snapper‑TUI‑rust 🚀

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) 
[![Rust](https://img.shields.io/badge/built_with-Rust-d65d0e.svg)](https://www.rust-lang.org/)

A modern, **feature‑rich** Terminal User Interface (TUI) for **Snapper** written in **Rust**. Manage Btrfs/LVM snapshots with speed, safety, and a cyber‑punk aesthetic.

![Demo](snapper-TUI-rust.gif)

## ✨ Features

- **🖥️ Modern UI** – Cyber‑punk theme with smooth animations.
- **📊 Interactive Dashboard** – Sortable, scrollable table of snapshots.
- **🛠️ CRUD Operations**
  - `c` – Create snapshots with custom description.
  - `d` – Delete (single or batch via multi‑selection).
  - `a` – Apply / rollback snapshots.
- **🔎 Search & Filter** – Instant filtering (`/`) by description, type, user, or ID.
- **🔍 Detailed Inspection** – View status, metadata, and config of any snapshot.
- **⚠️ Safety First** – Confirmation dialogs for destructive actions.
- **⚡ Async Performance** – Background processing keeps UI responsive.
- **🖱️ Full Mouse Support** – Click, scroll, and select with the mouse.

## 🛠️ Prerequisites

- Linux system with **Snapper** installed and configured.
- Root / sudo privileges (required for most snapshot operations).
- Rust toolchain (`rustc` & `cargo`).

## 📦 Installation

### From Source

```bash
# 1️⃣ Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2️⃣ Clone the repository
git clone https://github.com/ulolol/snapper-TUI-rust.git
cd snapper-TUI-rust

# 3️⃣ Build the binary (release mode)
cargo build --release

# 4️⃣ Run (usually needs sudo)
sudo ./target/release/snapper-TUI-rust
```

### Pre‑built Binaries (optional)
> sudo ./snapper-TUI-rust

I. Use pre-built binary:
   ```bash
   sudo ./snapper-TUI-rust
   ```

## ⌨️ Keybindings

| Key | Action |
|:---|:---|
| `q` / `Q` | Quit application |
| `c` / `C` | **Create** a new snapshot |
| `d` / `D` | **Delete** selected snapshot(s) |
| `a` / `A` | **Apply** (rollback) to selected snapshot |
| `r` / `R` | **Refresh** snapshot list |
| `s` / `S` | Get **Status** of selected snapshot |
| `/` | **Filter** snapshots |
| `Space` | **Toggle Selection** (batch ops) |
| `↑` / `↓` | Navigate list |
| `1`‑`5` | Sort by column (Number, Type, Date, User, Space) |
| `Esc` | Cancel popup / Clear filter |

## 🏗️ Architecture Overview

- **UI Layer** – Powered by `ratatui` & `crossterm` for terminal rendering. `tachyonfx` and `color-to-tui` for visual goodies.
- **State Management** – Central `AppState` struct holds snapshot list, selection state, and loading overlay.
- **Async Workers** – Snapshot operations run in separate threads, communicating via channels to keep the UI non‑blocking.
- **Snapper Wrapper** – Thin Rust wrapper around Snapper CLI (`snapper list`, `snapper create`, etc.) handling parsing and error mapping.

## 📚 Usage Example

```bash
# List snapshots (read‑only)
snapper list

# Launch the TUI (requires sudo for write ops)
sudo snapper‑TUI‑rust
```

Inside the UI, press `c` to create a snapshot, `d` to delete, `a` to apply, and use `/` to filter.

## 🤝 Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

1. Fork the repo.
2. Create a feature branch (`git checkout -b feat/awesome-feature`).
3. Open a PR describing your changes.

## 📄 License

This project is licensed under the [MIT License](LICENSE).

---
© 2025 Vidish
