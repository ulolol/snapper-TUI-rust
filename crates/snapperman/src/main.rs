use std::sync::Arc;

use gpui_kit::component::TitleBar;
use gpui_kit::{AppContext as _, Bounds, Size, WindowBounds, WindowOptions, px};
use snapper_core::backend::{ElevationStrategy, MockSnapperBackend, SnapperBackend, SnapperCliBackend};
use snapperman::actions;
use snapperman::state::{AppPrefs, WorkspaceModel};
use snapperman::theme;
use snapperman::views::SnapperShell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackendMode {
    Auto,
    Mock,
    Cli,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Dbus,
}

fn parse_args() -> BackendMode {
    let mut mode = BackendMode::Auto;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--mock" => mode = BackendMode::Mock,
            "--cli" => mode = BackendMode::Cli,
            "--dbus" => mode = BackendMode::Dbus,
            "--help" | "-h" => {
                println!(
                    "snapperman — GPU-accelerated Snapper Btrfs snapshot manager\n\n\
                     USAGE:\n  snapperman [OPTIONS]\n\n\
                     OPTIONS:\n  \
                       --mock    Run against the deterministic in-memory mock backend (default off-Linux)\n  \
                       --dbus    Connect directly via org.opensuse.Snapper D-Bus (Linux only)\n  \
                       --cli     Force the snapper CLI backend with pkexec fallback\n  \
                       -h, --help Show this help message"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown option: {other}");
                std::process::exit(2);
            }
        }
    }
    mode
}

fn build_backend(mode: BackendMode) -> Arc<dyn SnapperBackend> {
    match mode {
        BackendMode::Mock => Arc::new(MockSnapperBackend::seeded()),
        BackendMode::Cli => Arc::new(SnapperCliBackend::new(ElevationStrategy::PkexecFallback)),
        BackendMode::Dbus => {
            #[cfg(target_os = "linux")]
            {
                match snapper_core::backend::SnapperDbusBackend::connect() {
                    Ok(dbus) => Arc::new(dbus),
                    Err(err) => {
                        eprintln!(
                            "Warning: failed to connect to org.opensuse.Snapper D-Bus ({err}); falling back to CLI."
                        );
                        Arc::new(SnapperCliBackend::new(ElevationStrategy::PkexecFallback))
                    }
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                eprintln!(
                    "D-Bus backend is only available on Linux builds with the `dbus` feature; using mock backend."
                );
                Arc::new(MockSnapperBackend::seeded())
            }
        }
        BackendMode::Auto => {
            #[cfg(target_os = "linux")]
            {
                if let Ok(dbus) = snapper_core::backend::SnapperDbusBackend::connect() {
                    return Arc::new(dbus);
                }
                Arc::new(SnapperCliBackend::new(ElevationStrategy::PkexecFallback))
            }
            #[cfg(not(target_os = "linux"))]
            {
                Arc::new(MockSnapperBackend::seeded())
            }
        }
    }
}

fn main() {
    let mode = parse_args();
    let backend = build_backend(mode);
    let prefs = AppPrefs::load();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::init_themes(cx, prefs.theme);
            actions::bind_keys(cx);

            let workspace = cx.new(|cx| WorkspaceModel::new(backend.clone(), cx));

            let bounds = Bounds::centered(None, Size::new(px(1340.), px(860.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(Size::new(px(960.), px(640.))),
                ..TitleBar::window_options()
            };

            let ws_for_win = workspace.clone();
            let prefs_for_win = prefs.clone();
            gpui_kit::open_window(options, cx, move |window, cx| {
                window.set_window_title("snapperman — Snapper Snapshot Manager");
                cx.new(|cx| SnapperShell::new(ws_for_win.clone(), prefs_for_win.clone(), window, cx))
            })
            .expect("failed to open snapperman window");

            workspace.update(cx, |ws, cx| ws.refresh(cx));
        });
}
