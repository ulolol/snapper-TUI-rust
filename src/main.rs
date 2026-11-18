mod app;
mod data;
mod ui;

use crate::{
    app::{ActionButton, App, AsyncResult, SortKey},
    ui as app_ui,
};
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
            KeyModifiers, MouseButton, MouseEventKind,
        },
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::Rect,
    Terminal,
};
use std::{io, sync::mpsc, thread, time::Duration};

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

fn spawn_refresh(app: &mut App, loading_msg: &str) {
    app.loading = true;
    app.loading_message = loading_msg.to_string();
    let (tx, rx) = mpsc::channel();
    app.rx = Some(rx);
    thread::spawn(move || {
        let res = crate::data::list_snapshots()
            .map(AsyncResult::Snapshots)
            .map_err(|e| e.to_string());
        let _ = tx.send(res);
    });
}

fn spawn_status_fetch(app: &mut App, blocking_overlay: bool) {
    if app.loading {
        return;
    }
    if let Some(snap) = app.get_selected_snapshot().cloned() {
        app.status_request_id = app.status_request_id.wrapping_add(1);
        let request_id = app.status_request_id;
        app.status_text.clear();
        app.message = format!("⏳ Fetching status for {}...", snap.number);
        if blocking_overlay {
            app.loading = true;
            app.loading_message = format!("Fetching status for {}...", snap.number);
        }
        let (tx, rx) = mpsc::channel();
        app.rx = Some(rx);
        thread::spawn(move || {
            let res = crate::data::get_snapshot_status(&snap)
                .map(|text| AsyncResult::Status { request_id, text })
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
    }
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

fn trigger_status(app: &mut App) {
    if app.get_selected_count() > 0 {
        app.message = String::from(
            "❌ Error: Cannot get status with multi-selection active. Clear selections first.",
        );
    } else {
        spawn_status_fetch(app, true);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut guard = TerminalGuard::new()?;
    let mut app = App::new();

    // Start initial load in a separate thread
    spawn_refresh(&mut app, "Loading...");

    let res = run_app(&mut guard.terminal, &mut app);
    drop(guard);

    if let Err(err) = res {
        eprintln!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| app_ui::draw(f, app))?;

        // Check for threaded results
        if let Some(rx) = &app.rx
            && let Ok(result) = rx.try_recv()
        {
                app.loading = false;
                app.rx = None;
                match result {
                    Ok(AsyncResult::Snapshots(snapshots)) => {
                        app.snapshots = snapshots;
                        app.sort_snapshots();
                        app.message = format!("✅ Loaded {} snapshots.", app.snapshots.len());
                    }
                    Ok(AsyncResult::Create(name)) => {
                        app.message = format!("✅ Snapshot created: {}", name);
                        spawn_refresh(app, "Refreshing...");
                    }
                    Ok(AsyncResult::Delete { success, fail }) => {
                        app.handle_delete_result(success, fail);
                        spawn_refresh(app, "Refreshing...");
                    }
                    Ok(AsyncResult::Apply(number)) => {
                        app.message =
                            format!("✅ Snapshot {} applied. Reboot to take effect.", number);
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

        // Handle events
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }

                    // Always allow Ctrl+C to exit cleanly
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        return Ok(());
                    }

                    // Splash Screen Handling
                    if app.show_splash {
                        if key.kind == KeyEventKind::Press {
                            app.show_splash = false;
                        }
                        continue;
                    }

                    // Ignore other input while a blocking operation is running
                    if app.loading {
                        continue;
                    }

                    // Popup Handling
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
                                        app.rx = Some(rx);

                                        thread::spawn(move || {
                                            let mut success_count = 0;
                                            let mut error_count = 0;

                                            for (config, number) in targets {
                                                match crate::data::delete_snapshot(&config, number)
                                                {
                                                    Ok(_) => success_count += 1,
                                                    Err(_) => error_count += 1,
                                                }
                                            }

                                            let _ = tx.send(Ok(AsyncResult::Delete {
                                                success: success_count,
                                                fail: error_count,
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
                                    if let Some((config, number)) = app.get_target_for_apply() {
                                        app.loading = true;
                                        app.loading_message =
                                            format!("Applying snapshot {}...", number);

                                        let (tx, rx) = mpsc::channel();
                                        app.rx = Some(rx);

                                        thread::spawn(move || {
                                            let res =
                                                crate::data::rollback_snapshot(&config, number)
                                                    .map(|_| AsyncResult::Apply(number))
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
                                    app.loading = true;
                                    app.loading_message = String::from("Creating snapshot...");

                                    let (tx, rx) = mpsc::channel();
                                    app.rx = Some(rx);

                                    thread::spawn(move || {
                                        let res = crate::data::create_snapshot(&input)
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
                            spawn_refresh(app, "Refreshing...");
                        }
                        KeyCode::Char('a') | KeyCode::Char('A')
                            if key.kind == KeyEventKind::Press =>
                        {
                            trigger_apply(app);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.next();
                            spawn_status_fetch(app, false);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.previous();
                            spawn_status_fetch(app, false);
                        }
                        KeyCode::PageDown => {
                            app.page_down(10);
                            spawn_status_fetch(app, false);
                        }
                        KeyCode::PageUp => {
                            app.page_up(10);
                            spawn_status_fetch(app, false);
                        }
                        KeyCode::Char('d') | KeyCode::Char('D')
                            if key.kind == KeyEventKind::Press =>
                        {
                            if !app.get_targets_for_delete().is_empty() {
                                app.show_delete_popup = true;
                            }
                        }
                        KeyCode::Char('s') | KeyCode::Char('S')
                            if key.kind == KeyEventKind::Press =>
                        {
                            trigger_status(app);
                        }
                        KeyCode::Char(' ') if key.kind == KeyEventKind::Press => {
                            app.toggle_selection();
                        }
                        // Sorting keybinds
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
                                spawn_status_fetch(app, false);
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
                                    ActionButton::Delete => {
                                        if !app.get_targets_for_delete().is_empty() {
                                            app.show_delete_popup = true;
                                        }
                                    }
                                    ActionButton::Apply => trigger_apply(app),
                                    ActionButton::Filter => app.filtering = true,
                                    ActionButton::Status => trigger_status(app),
                                    ActionButton::Refresh => spawn_refresh(app, "Refreshing..."),
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
                                        // Column widths: 8, 10, 22, 12, 12 with column_spacing = 1
                                        if col_x < 9 {
                                            app.set_sort_key(SortKey::Number);
                                        } else if col_x < 20 {
                                            app.set_sort_key(SortKey::Type);
                                        } else if col_x < 43 {
                                            app.set_sort_key(SortKey::Date);
                                        } else if col_x < 56 {
                                            app.set_sort_key(SortKey::User);
                                        } else if col_x < 69 {
                                            app.set_sort_key(SortKey::UsedSpace);
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
                                        spawn_status_fetch(app, false);
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

        // Tick animations
        app.on_tick();
    }
}
