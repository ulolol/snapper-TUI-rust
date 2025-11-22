# snapper-TUI-rust

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/built_with-Rust-d65d0e.svg)

A modern, feature-rich Terminal User Interface (TUI) for **Snapper**, written in Rust. Manage your Btrfs/LVM snapshots with ease, style, and speed.

![Demo](snapper-TUI-rust.gif)

## 🚀 Features

*   **Modern UI**: A polished "Cyberpunk" aesthetic with smooth animations and a responsive layout.
*   **Interactive Dashboard**: Browse all your system snapshots in a sortable, scrollable table.
*   **CRUD Operations**:
    *   **Create** new snapshots with custom descriptions.
    *   **Delete** snapshots (supports single or batch deletion via multi-selection).
    *   **Rollback/Apply** snapshots to revert your system to a previous state.
*   **Search & Filter**: Quickly find snapshots by description, type, user, or ID using the built-in filter (`/`).
*   **Detailed Inspection**: View detailed status, metadata, and configuration for any snapshot.
*   **Safety First**: Confirmation dialogs for destructive actions (delete, rollback) to prevent accidents.
*   **Async Performance**: Background processing ensures the UI never freezes during long disk operations.
*   **Mouse Support**: Full mouse interaction for selecting, scrolling, and clicking buttons.

## 🛠️ Prerequisites

*   **Linux** system.
*   **Snapper** installed and configured.
*   **Root/Sudo** privileges are typically required to manage snapshots.

## 📦 Installation

### From Source

1.  Ensure you have Rust and Cargo installed.
2.  Clone the repository:
    ```bash
    git clone https://github.com/Vidish/snapper-TUI-rust.git
    cd snapper-TUI-rust
    ```
3.  Build and run:
    ```bash
    cargo build --release
    sudo ./target/release/snapper-TUI-rust
    ```
    *(Note: `sudo` is usually required for snapper commands)*

## ⌨️ Keybindings

| Key | Action |
| :--- | :--- |
| `q` / `Q` | Quit application |
| `c` / `C` | **Create** a new snapshot |
| `d` / `D` | **Delete** selected snapshot(s) |
| `a` / `A` | **Apply** (rollback) to selected snapshot |
| `r` / `R` | **Refresh** snapshot list |
| `s` / `S` | Get **Status** of selected snapshot |
| `/` | **Filter** snapshots |
| `Space` | **Toggle Selection** (for batch operations) |
| `↑` / `↓` | Navigate list |
| `1` - `5` | Sort by column (Number, Type, Date, User, Space) |
| `Esc` | Cancel popup / Clear filter |

## 🤝 Contributing

Contributions are welcome! Feel free to submit a Pull Request.

## 📄 License

This project is licensed under the [MIT License](LICENSE).

Copyright (c) 2025 Vidish
