mod app;
mod ui;

use crate::{
    app::{ActionButton, App, AsyncResult},
    ui as app_ui,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
            KeyModifiers, MouseButton, MouseEventKind,
        },
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::Rect,
};
use snapper_core::backend::{
    ElevationStrategy, MockSnapperBackend, SnapperBackend, SnapperCliBackend,
};
use snapper_core::model::CreateSnapshotRequest;
use snapper_core::query::{SortKey, resolve_default_compare_range};
use std::{io, sync::Arc, sync::mpsc, thread, time::Duration};

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(err) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            return Err(err);
        }
        let backend = CrosstermBackend::new(stdout);
        match Terminal::new(backend) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(err) => {
                let mut stdout = io::stdout();
                let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
                let _ = disable_raw_mode();
                Err(err)
            }
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}

fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x
        && col < r.x.saturating_add(r.width)
        && row >= r.y
        && row < r.y.saturating_add(r.height)
}

fn spawn_refresh(app: &mut App, backend: &Arc<dyn SnapperBackend>, loading_msg: &str) {
    app.loading = true;
    app.loading_message = loading_msg.to_string();
    let (tx, rx) = mpsc::channel();
    app.op_rx = Some(rx);
    let backend = Arc::clone(backend);
    thread::spawn(move || {
        let res = backend
            .list_snapshots(None, true)
            .map(AsyncResult::Snapshots)
            .map_err(|e| e.to_string());
        let _ = tx.send(res);
    });
}

fn spawn_status_fetch(app: &mut App, backend: &Arc<dyn SnapperBackend>, blocking_overlay: bool) {
    if app.loading {
        return;
    }
    if let Some(snap) = app.get_selected_snapshot().cloned() {
        app.status_request_id = app.status_request_id.wrapping_add(1);
        let request_id = app.status_request_id;
        app.status_text.clear();

        let Some((from, to)) = resolve_default_compare_range(&snap, &app.snapshots) else {
            app.message = format!("ℹ️ No comparable adjacent snapshot for {}.", snap.key());
            return;
        };

        app.message = format!("⏳ Fetching status for {} ({}..{})...", snap.key(), from, to);
        if blocking_overlay {
            app.loading = true;
            app.loading_message = format!("Fetching status for {}...", snap.key());
        }

        let (tx, rx) = mpsc::channel();
        if blocking_overlay {
            app.op_rx = Some(rx);
        } else {
            app.status_rx = Some(rx);
        }

        let backend = Arc::clone(backend);
        thread::spawn(move || {
            let res = backend
                .snapshot_status(&snap.config, from, to)
                .map(|changes| {
                    let text = if changes.is_empty() {
                        format!("No file changes in {}..{}.", from, to)
                    } else {
                        changes
                            .into_iter()
                            .map(|c| format!("{} {}", c.flags.to_flag_string(), c.path))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    AsyncResult::Status { request_id, text }
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }
}

fn trigger_delete(app: &mut App) {
    let targets = app.get_targets_for_delete();
    if targets.is_empty() {
        return;
    }
    let protected_count = app.count_protected_delete_targets();
    if protected_count == targets.len() {
        app.message = String::from(
            "❌ Protected snapshot(s) (#0 / active / default) cannot be deleted.",
        );
        return;
    }
    app.show_delete_popup = true;
}

fn trigger_apply(app: &mut App) {
    if app.get_selected_count() > 0 {
        app.message = String::from(
            "❌ Error: Cannot apply with multi-selection active. Clear selections first (Space or Esc).",
        );
    } else if app.get_selected_snapshot().is_some() {
        app.show_apply_popup = true;
    }
}

fn trigger_status(app: &mut App, backend: &Arc<dyn SnapperBackend>) {
    if app.get_selected_count() > 0 {
        app.message = String::from(
            "❌ Error: Cannot get status with multi-selection active. Clear selections first.",
        );
    } else {
        spawn_status_fetch(app, backend, true);
    }
}

fn handle_async_result(
    app: &mut App,
    backend: &Arc<dyn SnapperBackend>,
    result: Result<AsyncResult, String>,
) {
    match result {
        Ok(AsyncResult::Snapshots(snapshots)) => {
            app.snapshots = snapshots;
            app.sort_snapshots();
            app.message = format!("✅ Loaded {} snapshots.", app.snapshots.len());
            spawn_status_fetch(app, backend, false);
        }
        Ok(AsyncResult::Create(name)) => {
            app.message = format!("✅ Snapshot created: {}", name);
            spawn_refresh(app, backend, "Refreshing...");
        }
        Ok(AsyncResult::Delete {
            success,
            fail,
            first_error,
        }) => {
            app.handle_delete_result(success, fail, first_error);
            spawn_refresh(app, backend, "Refreshing...");
        }
        Ok(AsyncResult::Apply(number)) => {
            app.message = format!("✅ Snapshot {} applied. Reboot to take effect.", number);
        }
        Ok(AsyncResult::Status { request_id, text }) => {
            if request_id == app.status_request_id {
                app.status_text = text;
                app.message = String::from("✅ Status loaded.");
                app.status_scroll = 0;
            }
        }
        Err(e) => {
            app.message = format!("❌ Error: {}", e);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "snapper-TUI-rust — Terminal UI for Snapper snapshots\n\n\
             USAGE:\n  snapper-TUI-rust [OPTIONS]\n\n\
             OPTIONS:\n  \
               --mock      Run against deterministic mock snapshots\n  \
               -h, --help  Show this help message"
        );
        return Ok(());
    }

    let use_mock = std::env::args().any(|arg| arg == "--mock");
    let backend: Arc<dyn SnapperBackend> = if use_mock {
        Arc::new(MockSnapperBackend::seeded())
    } else {
        Arc::new(SnapperCliBackend::new(ElevationStrategy::SudoNonInteractive))
    };

    let mut guard = TerminalGuard::new()?;
    let mut app = App::new();

    spawn_refresh(&mut app, &backend, "Loading...");

    let res = run_app(&mut guard.terminal, &mut app, &backend);
    drop(guard);

    if let Err(err) = res {
        eprintln!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    backend: &Arc<dyn SnapperBackend>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| app_ui::draw(f, app))?;

        if let Some(rx) = &app.op_rx
            && let Ok(result) = rx.try_recv()
        {
            app.loading = false;
            app.op_rx = None;
            handle_async_result(app, backend, result);
        }

        if let Some(rx) = &app.status_rx
            && let Ok(result) = rx.try_recv()
        {
            app.status_rx = None;
            handle_async_result(app, backend, result);
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        return Ok(());
                    }

                    if app.show_splash {
                        if key.kind == KeyEventKind::Press {
                            app.show_splash = false;
                        }
                        continue;
                    }

                    if app.loading {
                        continue;
                    }

                    if app.show_delete_popup {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Enter => {
                                    let targets = app.get_targets_for_delete();
                                    if !targets.is_empty() {
                                        app.loading = true;
                                        app.loading_message =
                                            format!("Deleting {} snapshot(s)...", targets.len());

                                        let (tx, rx) = mpsc::channel();
                                        app.op_rx = Some(rx);
                                        let backend = Arc::clone(backend);

                                        thread::spawn(move || {
                                            let report = backend.delete_snapshots(&targets);
                                            let first_error =
                                                report.failed.first().map(|(_, err)| err.clone());
                                            let _ = tx.send(Ok(AsyncResult::Delete {
                                                success: report.success_count(),
                                                fail: report.fail_count(),
                                                first_error,
                                            }));
                                        });
                                    }
                                    app.show_delete_popup = false;
                                }
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    app.show_delete_popup = false;
                                }
                                _ => {}
                            }
                        }
                        continue;
                    }

                    if app.show_apply_popup {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Enter => {
                                    if let Some(target) = app.get_target_for_apply() {
                                        app.loading = true;
                                        app.loading_message =
                                            format!("Applying snapshot {}...", target.number);

                                        let (tx, rx) = mpsc::channel();
                                        app.op_rx = Some(rx);
                                        let backend = Arc::clone(backend);
                                        let num = target.number;

                                        thread::spawn(move || {
                                            let res = backend
                                                .rollback_snapshot(&target)
                                                .map(|_| AsyncResult::Apply(num))
                                                .map_err(|e| e.to_string());
                                            let _ = tx.send(res);
                                        });
                                    }
                                    app.show_apply_popup = false;
                                }
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    app.show_apply_popup = false;
                                }
                                _ => {}
                            }
                        }
                        continue;
                    }

                    if app.show_create_popup {
                        match key.code {
                            KeyCode::Enter if key.kind == KeyEventKind::Press => {
                                let input = app.create_input.trim().to_string();
                                if !input.is_empty() {
                                    let config = app
                                        .get_selected_snapshot()
                                        .map(|s| s.config.clone())
                                        .filter(|c| !c.is_empty())
                                        .unwrap_or_else(|| "root".to_string());

                                    app.loading = true;
                                    app.loading_message = String::from("Creating snapshot...");

                                    let (tx, rx) = mpsc::channel();
                                    app.op_rx = Some(rx);
                                    let backend = Arc::clone(backend);

                                    thread::spawn(move || {
                                        let req = CreateSnapshotRequest::single(config, input.clone());
                                        let res = backend
                                            .create_snapshot(&req)
                                            .map(|_| AsyncResult::Create(input))
                                            .map_err(|e| e.to_string());
                                        let _ = tx.send(res);
                                    });
                                    app.create_input.clear();
                                    app.show_create_popup = false;
                                }
                            }
                            KeyCode::Esc if key.kind == KeyEventKind::Press => {
                                app.show_create_popup = false;
                                app.create_input.clear();
                            }
                            KeyCode::Char(c) => {
                                app.create_input.push(c);
                            }
                            KeyCode::Backspace => {
                                app.create_input.pop();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if app.filtering {
                        match key.code {
                            KeyCode::Enter if key.kind == KeyEventKind::Press => {
                                app.filtering = false;
                            }
                            KeyCode::Esc if key.kind == KeyEventKind::Press => {
                                app.filtering = false;
                                app.filter_input.clear();
                                app.clamp_selection();
                            }
                            KeyCode::Char(c) => {
                                app.filter_input.push(c);
                                app.table_state.select(Some(0));
                                app.clamp_selection();
                            }
                            KeyCode::Backspace => {
                                app.filter_input.pop();
                                app.table_state.select(Some(0));
                                app.clamp_selection();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q')
                            if key.kind == KeyEventKind::Press =>
                        {
                            return Ok(());
                        }
                        KeyCode::Char('c') | KeyCode::Char('C')
                            if key.kind == KeyEventKind::Press =>
                        {
                            app.show_create_popup = true;
                        }
                        KeyCode::Char('/') if key.kind == KeyEventKind::Press => {
                            app.filtering = true;
                        }
                        KeyCode::Esc if key.kind == KeyEventKind::Press => {
                            if !app.filter_input.is_empty() {
                                app.filter_input.clear();
                                app.clamp_selection();
                            } else if app.get_selected_count() > 0 {
                                app.clear_selections();
                            }
                        }
                        KeyCode::Char('r') | KeyCode::Char('R')
                            if key.kind == KeyEventKind::Press =>
                        {
                            spawn_refresh(app, backend, "Refreshing...");
                        }
                        KeyCode::Char('a') | KeyCode::Char('A')
                            if key.kind == KeyEventKind::Press =>
                        {
                            trigger_apply(app);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.next();
                            spawn_status_fetch(app, backend, false);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.previous();
                            spawn_status_fetch(app, backend, false);
                        }
                        KeyCode::PageDown => {
                            app.page_down(10);
                            spawn_status_fetch(app, backend, false);
                        }
                        KeyCode::PageUp => {
                            app.page_up(10);
                            spawn_status_fetch(app, backend, false);
                        }
                        KeyCode::Char('d') | KeyCode::Char('D')
                            if key.kind == KeyEventKind::Press =>
                        {
                            trigger_delete(app);
                        }
                        KeyCode::Char('s') | KeyCode::Char('S')
                            if key.kind == KeyEventKind::Press =>
                        {
                            trigger_status(app, backend);
                        }
                        KeyCode::Char(' ') if key.kind == KeyEventKind::Press => {
                            app.toggle_selection();
                        }
                        KeyCode::Char('0') if key.kind == KeyEventKind::Press => {
                            app.set_sort_key(SortKey::Config);
                        }
                        KeyCode::Char('1') if key.kind == KeyEventKind::Press => {
                            app.set_sort_key(SortKey::Number);
                        }
                        KeyCode::Char('2') if key.kind == KeyEventKind::Press => {
                            app.set_sort_key(SortKey::Type);
                        }
                        KeyCode::Char('3') if key.kind == KeyEventKind::Press => {
                            app.set_sort_key(SortKey::Date);
                        }
                        KeyCode::Char('4') if key.kind == KeyEventKind::Press => {
                            app.set_sort_key(SortKey::User);
                        }
                        KeyCode::Char('5') if key.kind == KeyEventKind::Press => {
                            app.set_sort_key(SortKey::UsedSpace);
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    if app.show_splash
                        || app.loading
                        || app.show_delete_popup
                        || app.show_create_popup
                        || app.show_apply_popup
                    {
                        continue;
                    }

                    match mouse.kind {
                        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                            let is_scroll_up = matches!(mouse.kind, MouseEventKind::ScrollUp);
                            if rect_contains(app.ui_areas.details, mouse.column, mouse.row) {
                                app.scroll_details(is_scroll_up);
                            } else if rect_contains(app.ui_areas.status, mouse.column, mouse.row) {
                                app.scroll_status(is_scroll_up);
                            } else if rect_contains(app.ui_areas.table, mouse.column, mouse.row) {
                                if is_scroll_up {
                                    app.previous();
                                } else {
                                    app.next();
                                }
                                spawn_status_fetch(app, backend, false);
                            }
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            if let Some((_, action)) = app
                                .ui_areas
                                .footer_buttons
                                .iter()
                                .find(|(rect, _)| rect_contains(*rect, mouse.column, mouse.row))
                                .copied()
                            {
                                match action {
                                    ActionButton::Create => app.show_create_popup = true,
                                    ActionButton::Delete => trigger_delete(app),
                                    ActionButton::Apply => trigger_apply(app),
                                    ActionButton::Filter => app.filtering = true,
                                    ActionButton::Status => trigger_status(app, backend),
                                    ActionButton::Refresh => {
                                        spawn_refresh(app, backend, "Refreshing...")
                                    }
                                    ActionButton::Quit => return Ok(()),
                                }
                            } else if rect_contains(app.ui_areas.table, mouse.column, mouse.row) {
                                let table = app.ui_areas.table;
                                let inner_bottom =
                                    table.y.saturating_add(table.height).saturating_sub(1);

                                if mouse.row == app.ui_areas.table_header_row {
                                    // Border (1) + highlight_symbol "👉 " (3) = 4
                                    let content_start = table.x.saturating_add(4);
                                    if mouse.column >= content_start {
                                        let col_x = mouse.column - content_start;
                                        // Column widths: Cfg(8), #(8), Type(8), Date(20), User(10), Space(10), Desc(Min) with spacing = 1
                                        if col_x < 9 {
                                            app.set_sort_key(SortKey::Config);
                                        } else if col_x < 18 {
                                            app.set_sort_key(SortKey::Number);
                                        } else if col_x < 27 {
                                            app.set_sort_key(SortKey::Type);
                                        } else if col_x < 48 {
                                            app.set_sort_key(SortKey::Date);
                                        } else if col_x < 59 {
                                            app.set_sort_key(SortKey::User);
                                        } else if col_x < 70 {
                                            app.set_sort_key(SortKey::UsedSpace);
                                        } else {
                                            app.set_sort_key(SortKey::Description);
                                        }
                                    }
                                } else if mouse.row >= app.ui_areas.table_first_data_row
                                    && mouse.row < inner_bottom
                                {
                                    let row_offset =
                                        (mouse.row - app.ui_areas.table_first_data_row) as usize;
                                    let target_index = app.table_state.offset() + row_offset;
                                    if target_index < app.get_filtered_snapshots().len() {
                                        app.select_row(target_index);
                                        spawn_status_fetch(app, backend, false);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        app.on_tick();
    }
}
