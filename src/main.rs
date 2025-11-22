mod app;
mod data;
mod ui;

use std::{io, thread, time::Duration};
use std::sync::mpsc;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use crate::{app::{App, AsyncResult}, ui as app_ui}; // Renamed to avoid conflict

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new();
    
    // Start initial load in a separate thread
    let (tx, rx) = mpsc::channel();
    app.rx = Some(rx);
    thread::spawn(move || {
        let res = crate::data::list_snapshots()
            .map(AsyncResult::Snapshots)
            .map_err(|e| e.to_string());
        let _ = tx.send(res);
    });

    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| app_ui::draw(f, app))?;

        // Check for threaded results
        if let Some(rx) = &app.rx {
            if let Ok(result) = rx.try_recv() {
                app.loading = false;
                app.rx = None; // Stop checking
                match result {
                    Ok(AsyncResult::Snapshots(snapshots)) => {
                        app.snapshots = snapshots;
                        app.message = format!("✅ Loaded {} snapshots.", app.snapshots.len());
                        if !app.snapshots.is_empty() {
                            app.table_state.select(Some(0));
                        }
                    }
                    Ok(AsyncResult::Create(name)) => {
                        app.message = format!("✅ Snapshot created: {}", name);
                        // Trigger refresh
                        app.loading = true;
                        app.loading_message = String::from("Refreshing...");
                        let (tx, rx) = mpsc::channel();
                        app.rx = Some(rx);
                        thread::spawn(move || {
                            let res = crate::data::list_snapshots()
                                .map(AsyncResult::Snapshots)
                                .map_err(|e| e.to_string());
                            let _ = tx.send(res);
                        });
                    }
                    Ok(AsyncResult::Delete { success, fail }) => {
                        app.handle_delete_result(success, fail);
                        // Trigger refresh
                        app.loading = true;
                        app.loading_message = String::from("Refreshing...");
                        let (tx, rx) = mpsc::channel();
                        app.rx = Some(rx);
                        thread::spawn(move || {
                            let res = crate::data::list_snapshots()
                                .map(AsyncResult::Snapshots)
                                .map_err(|e| e.to_string());
                            let _ = tx.send(res);
                        });
                    }
                    Ok(AsyncResult::Apply(number)) => {
                        app.message = format!("✅ Snapshot {} applied. Reboot to take effect.", number);
                    }
                    Ok(AsyncResult::Status(status)) => {
                        app.status_text = status;
                        app.message = String::from("✅ Status loaded.");
                        app.status_scroll = 0;
                    }
                    Err(e) => {
                        app.message = format!("❌ Error: {}", e);
                    }
                }
            }
        }

        // Handle events
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    // Splash Screen Handling
                    if app.show_splash {
                        app.show_splash = false; // Dismiss on any key
                        continue;
                    }

                    // Popup Handling
                    if app.show_delete_popup {
                        match key.code {
                            KeyCode::Enter => {
                                let targets = app.get_targets_for_delete();
                                if !targets.is_empty() {
                                    app.loading = true;
                                    app.loading_message = format!("Deleting {} snapshot(s)...", targets.len());
                                    
                                    let (tx, rx) = mpsc::channel();
                                    app.rx = Some(rx);
                                    
                                    thread::spawn(move || {
                                        let mut success_count = 0;
                                        let mut error_count = 0;
                                        
                                        for number in targets {
                                            match crate::data::delete_snapshot(number) {
                                                Ok(_) => success_count += 1,
                                                Err(_) => error_count += 1,
                                            }
                                        }
                                        
                                        let res = Ok(AsyncResult::Delete { success: success_count, fail: error_count });
                                        let _ = tx.send(res);
                                    });
                                }
                                app.show_delete_popup = false;
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                app.show_delete_popup = false;
                            }
                            _ => {}
                        }
                        continue;
                    }
                    if app.show_apply_popup {
                        match key.code {
                            KeyCode::Enter => {
                                if let Some(number) = app.get_target_for_apply() {
                                    app.loading = true;
                                    app.loading_message = format!("Applying snapshot {}...", number);
                                    
                                    let (tx, rx) = mpsc::channel();
                                    app.rx = Some(rx);
                                    
                                    thread::spawn(move || {
                                        let res = crate::data::rollback_snapshot(number)
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
                        continue;
                    }
                    if app.show_create_popup {
                        match key.code {
                            KeyCode::Enter => {
                                if !app.create_input.is_empty() {
                                    app.loading = true;
                                    app.loading_message = String::from("Creating snapshot...");
                                    
                                    let input = app.create_input.clone();
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
                            KeyCode::Esc => {
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
                            KeyCode::Enter => {
                                app.filtering = false;
                            }
                            KeyCode::Esc => {
                                app.filtering = false;
                                app.filter_input.clear();
                                app.table_state.select(Some(0));
                            }
                            KeyCode::Char(c) => {
                                app.filter_input.push(c);
                                app.table_state.select(Some(0));
                            }
                            KeyCode::Backspace => {
                                app.filter_input.pop();
                                app.table_state.select(Some(0));
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            app.show_create_popup = true;
                        }
                        KeyCode::Char('/') => {
                            app.filtering = true;
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            app.loading = true;
                            app.loading_message = String::from("Refreshing...");
                            app.snapshots.clear();
                            
                            let (tx, rx) = mpsc::channel();
                            app.rx = Some(rx);
                            thread::spawn(move || {
                                let res = crate::data::list_snapshots()
                                    .map(AsyncResult::Snapshots)
                                    .map_err(|e| e.to_string());
                                let _ = tx.send(res);
                            });
                        }
                        KeyCode::Char('a') | KeyCode::Char('A') => {
                            if app.get_selected_count() > 0 {
                                app.message = "❌ Error: Cannot apply with multi-selection active. Clear selections first (select with space to deselect).".to_string();
                            } else {
                                app.show_apply_popup = true;
                            }
                        }
                        KeyCode::Down => {
                            app.next();
                            app.get_status_selected_snapshot(); // Auto-show status
                        }
                        KeyCode::Up => {
                            app.previous();
                            app.get_status_selected_snapshot(); // Auto-show status
                        }
                        KeyCode::Char('d') | KeyCode::Char('D') => app.show_delete_popup = true,
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            if app.get_selected_count() > 0 {
                                app.message = "❌ Error: Cannot get status with multi-selection active. Clear selections first.".to_string();
                            } else {
                                if let Some(snap) = app.get_selected_snapshot().cloned() {
                                    app.loading = true;
                                    app.loading_message = format!("Fetching status for {}...", snap.number);
                                    let (tx, rx) = mpsc::channel();
                                    app.rx = Some(rx);
                                    thread::spawn(move || {
                                        let res = crate::data::get_snapshot_status(&snap)
                                            .map(AsyncResult::Status)
                                            .map_err(|e| e.to_string());
                                        let _ = tx.send(res);
                                    });
                                }
                            }
                        }
                        KeyCode::Char(' ') => app.toggle_selection(),
                        // Sorting keybinds
                        KeyCode::Char('1') => app.set_sort_key(crate::app::SortKey::Number),
                        KeyCode::Char('2') => app.set_sort_key(crate::app::SortKey::Type),
                        KeyCode::Char('3') => app.set_sort_key(crate::app::SortKey::Date),
                        KeyCode::Char('4') => app.set_sort_key(crate::app::SortKey::User),
                        KeyCode::Char('5') => app.set_sort_key(crate::app::SortKey::UsedSpace),
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        event::MouseEventKind::ScrollDown | event::MouseEventKind::ScrollUp => {
                            let term_size = terminal.size()?;
                            let is_scroll_up = matches!(mouse.kind, event::MouseEventKind::ScrollUp);
                            
                            // Calculate layout boundaries
                            // Calculate layout boundaries
                            // Layout: TopGap(1) + Header(5) + Gap(1) + Main + Gap(1) + Footer(3) + BottomGap(1)
                            let header_offset = 7; // 1 + 5 + 1
                            let footer_height = 3;
                            let bottom_gap = 1;
                            let main_area_start = header_offset;
                            let main_area_end = term_size.height.saturating_sub(footer_height + bottom_gap + 1); // +1 for the gap above footer
                            
                            // Check if mouse is in main area
                            if mouse.row >= main_area_start && mouse.row < main_area_end {
                                // Main area is split 50/50 horizontally
                                let half_width = term_size.width / 2;
                                
                                // Right panel (Details + Status)
                                if mouse.column >= half_width {
                                    // Right panel is split vertically: 40% Details, 60% Status
                                    let right_panel_height = main_area_end - main_area_start;
                                    let details_height = (right_panel_height * 40) / 100;
                                    let details_end_row = main_area_start + details_height;
                                    
                                    if mouse.row < details_end_row {
                                        // Mouse is in Details pane
                                        app.scroll_details(is_scroll_up);
                                    } else {
                                        // Mouse is in Status pane
                                        app.scroll_status(is_scroll_up);
                                    }
                                }
                                // Left panel (table) - no scrolling needed
                            }
                        }
                        event::MouseEventKind::Down(event::MouseButton::Left) => {
                            let term_size = terminal.size()?;
                            // Footer starts at Height - BottomGap(1) - Footer(3)
                            let footer_row = term_size.height.saturating_sub(4);
                            let is_in_footer = mouse.row >= footer_row && mouse.row < term_size.height.saturating_sub(1);
                            
                            // Layout: TopGap(1) + Header(5) + Gap(1) = 7
                            let main_area_start = 7;
                            
                            if is_in_footer {
                                // Footer button clicks
                                let col = mouse.column;
                                if col >= 10 && col < 20 { app.show_delete_popup = true; }
                                else if col >= 20 && col < 30 { app.show_apply_popup = true; }
                                else if col >= 30 && col < 40 { 
                                    if let Some(snap) = app.get_selected_snapshot().cloned() {
                                        app.loading = true;
                                        app.loading_message = format!("Fetching status for {}...", snap.number);
                                        let (tx, rx) = mpsc::channel();
                                        app.rx = Some(rx);
                                        thread::spawn(move || {
                                            let res = crate::data::get_snapshot_status(&snap)
                                                .map(AsyncResult::Status)
                                                .map_err(|e| e.to_string());
                                            let _ = tx.send(res);
                                        });
                                    }
                                }
                                else if col >= 40 && col < 50 { 
                                    app.loading = true;
                                    app.loading_message = String::from("Refreshing...");
                                    app.snapshots.clear();
                                    let (tx, rx) = mpsc::channel();
                                    app.rx = Some(rx);
                                    thread::spawn(move || {
                                        let res = crate::data::list_snapshots()
                                            .map(AsyncResult::Snapshots)
                                            .map_err(|e| e.to_string());
                                        let _ = tx.send(res);
                                    });
                                }
                                else if col >= 50 && col < 60 { return Ok(()); }
                            } else if mouse.row >= main_area_start && mouse.row < footer_row {
                                // Main area - check if left panel (table)
                                let half_width = term_size.width / 2;
                                let left_padding = 2;
                                
                                if mouse.column >= left_padding && mouse.column < half_width {
                                    // Adjust column for padding
                                    let effective_col = mouse.column - left_padding;
                                    // Table block starts at main_area_start
                                    // Border = 1 row, Header = 1 row
                                    // Table block starts at main_area_start
                                    // Border = 1 row, Header = 1 row
                                    let table_border_top = main_area_start;
                                    let table_header_row = table_border_top + 1;
                                    let first_data_row = table_header_row + 1;
                                    
                                    if mouse.row == table_header_row {
                                        // Clicked on table header - determine column for sorting
                                        let col_x = effective_col;
                                        
                                        // Column boundaries based on UI constraints:
                                        // Border: 1
                                        // Col 1 (Number): 8 -> End 9
                                        // Col 2 (Type): 10 -> End 19
                                        // Col 3 (Date): 22 -> End 41
                                        // Col 4 (User): 12 -> End 53
                                        // Col 5 (Space): 12 -> End 65
                                        if col_x < 9 {
                                            app.set_sort_key(crate::app::SortKey::Number);
                                        } else if col_x < 19 {
                                            app.set_sort_key(crate::app::SortKey::Type);
                                        } else if col_x < 41 {
                                            app.set_sort_key(crate::app::SortKey::Date);
                                        } else if col_x < 53 {
                                            app.set_sort_key(crate::app::SortKey::User);
                                        } else if col_x < 65 {
                                            app.set_sort_key(crate::app::SortKey::UsedSpace);
                                        }
                                    } else if mouse.row >= first_data_row {
                                        // Clicked on table body - select row
                                        let row_offset = mouse.row.saturating_sub(first_data_row);
                                        let target_index = row_offset as usize;
                                        
                                        if target_index < app.snapshots.len() {
                                            app.table_state.select(Some(target_index));
                                            app.get_status_selected_snapshot(); // Auto-show status
                                        }
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
