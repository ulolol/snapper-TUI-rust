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
use crate::{app::App, ui as app_ui}; // Renamed to avoid conflict

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
        let res = crate::data::list_snapshots().map_err(|e| e.to_string());
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
                    Ok(snapshots) => {
                        app.snapshots = snapshots;
                        app.message = format!("Loaded {} snapshots.", app.snapshots.len());
                        if !app.snapshots.is_empty() {
                            app.table_state.select(Some(0));
                        }
                    }
                    Err(e) => {
                        app.message = format!("Error loading snapshots: {}", e);
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
                                app.delete_selected_snapshot();
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
                                app.apply_selected_snapshot();
                                app.show_apply_popup = false;
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                app.show_apply_popup = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            app.loading = true;
                            app.message = String::from("Refreshing...");
                            app.snapshots.clear();
                            
                            let (tx, rx) = mpsc::channel();
                            app.rx = Some(rx);
                            thread::spawn(move || {
                                let res = crate::data::list_snapshots().map_err(|e| e.to_string());
                                let _ = tx.send(res);
                            });
                        }
                        KeyCode::Char('a') | KeyCode::Char('A') => app.show_apply_popup = true,
                        KeyCode::Down => app.next(),
                        KeyCode::Up => app.previous(),
                        KeyCode::Char('d') | KeyCode::Char('D') => app.show_delete_popup = true,
                        KeyCode::Char('s') | KeyCode::Char('S') => app.get_status_selected_snapshot(),
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        event::MouseEventKind::ScrollDown | event::MouseEventKind::ScrollUp => {
                            let term_size = terminal.size()?;
                            let is_scroll_up = matches!(mouse.kind, event::MouseEventKind::ScrollUp);
                            
                            // Calculate layout boundaries
                            let header_height = 3;
                            let footer_height = 3;
                            let main_area_start = header_height;
                            let main_area_end = term_size.height.saturating_sub(footer_height);
                            
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
                            let footer_row = term_size.height.saturating_sub(3);
                            let is_in_footer = mouse.row >= footer_row;
                            
                            // Simple layout assumptions for hit testing
                            // Header: 3 rows (0, 1, 2)
                            // Main: Row 3 to footer_row - 1
                            // Footer: footer_row to end
                            
                            if is_in_footer {
                                // Simple hit testing for buttons based on text position
                                // Actions: [D]elete [a]pply [S]tatus [r]efresh [q]uit
                                let col = mouse.column;
                                if col >= 10 && col < 20 { app.show_delete_popup = true; }
                                else if col >= 20 && col < 30 { app.show_apply_popup = true; }
                                else if col >= 30 && col < 40 { app.get_status_selected_snapshot(); }
                                else if col >= 40 && col < 50 { 
                                    // Refresh logic duplicated for now
                                    app.loading = true;
                                    app.message = String::from("Refreshing...");
                                    app.snapshots.clear();
                                    let (tx, rx) = mpsc::channel();
                                    app.rx = Some(rx);
                                    thread::spawn(move || {
                                        let res = crate::data::list_snapshots().map_err(|e| e.to_string());
                                        let _ = tx.send(res);
                                    });
                                }
                                else if col >= 50 && col < 60 { return Ok(()); }
                            } else if mouse.row >= 3 && mouse.row < footer_row {
                                // Main area
                                let width = term_size.width;
                                let is_in_left_panel = mouse.column < width / 2;
                                
                                if is_in_left_panel {
                                    // Snapshot Table Click
                                    // Header of table is 1 row (inside the block), block border is 1 row.
                                    // So table content starts at: Header(3) + Border(1) + TableHeader(1) = Row 5?
                                    // Let's approximate: Header is 3 rows. Table block starts at row 3.
                                    // Table block border top is row 3. Table header is row 4. First data row is row 5.
                                    
                                    let header_height = 3; // App header
                                    let table_header_height = 2; // Border + Header row
                                    let first_data_row = header_height + table_header_height;
                                    
                                    if mouse.row >= first_data_row {
                                        let index_in_view = (mouse.row - first_data_row) as usize;
                                        // We need to account for table scroll offset if we had it exposed.
                                        // For now, assuming list fits or simple scrolling:
                                        // Ratatui TableState doesn't easily expose offset without tracking it ourselves.
                                        // But we can try to select based on visual index if we assume top is 0 for now
                                        // or if we implement offset tracking in App.
                                        
                                        // Since we don't track offset yet, this is "best effort" for visible rows
                                        // assuming the table is scrolled to top or we track it.
                                        // TODO: Implement proper offset tracking in App::next/previous to make this perfect.
                                        if index_in_view < app.snapshots.len() {
                                            app.table_state.select(Some(index_in_view));
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
