use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};
use tachyonfx::{
    fx, Duration, Effect, EffectRenderer, Interpolation,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    // Splash Screen - simple custom implementation
    if app.show_splash {
        if let Some(start) = app.splash_start {
            if start.elapsed().as_secs() >= 2 {
                app.show_splash = false;
            } else {
                // Render simple centered splash
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(Style::default().bg(Color::Black).fg(Color::Cyan));
                f.render_widget(block, f.area());
                
                let text = vec![
                    Line::from(""),
                    Line::from(""),
                    Line::from(""),
                    Line::from(Span::styled(
                        "⚡ SNAPPER TUI ⚡",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Initializing System...",
                        Style::default().fg(Color::Gray),
                    )),
                ];
                
                let para = Paragraph::new(text).alignment(Alignment::Center);
                let center = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(40),
                        Constraint::Length(6),
                        Constraint::Percentage(40),
                    ])
                    .split(f.area())[1];
                f.render_widget(para, center);
                return;
            }
        }
    }

    // Initialize effect if not present
    if app.fx.is_none() {
        let effect = fx::fade_from(
            ratatui::style::Color::Black,
            ratatui::style::Color::Black,
            (Duration::from_millis(1000), Interpolation::Linear),
        );
        app.fx = Some(effect);
        app.fx_start = Some(std::time::Instant::now());
    }

    if app.loading && app.snapshots.is_empty() {
        draw_loading_screen(f, app);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(3), // Footer/Actions
            ])
            .split(f.area());

        draw_header(f, chunks[0]);
        draw_main(f, app, chunks[1]);
        draw_actions_bar(f, chunks[2]);
    }

    // Render TachyonFX effects
    if let Some(effect) = &mut app.fx {
        if let Some(start) = app.fx_start {
            f.render_effect(effect, f.area(), start.elapsed().into());
        }
    }

    // Custom Popups - render on top
    if app.show_delete_popup {
        draw_popup(
            f,
            "⚠ Delete Snapshot ⚠",
            "Are you sure you want to delete this snapshot?\n\nThis action cannot be undone.\n\n[Enter] Confirm  [Esc] Cancel",
            Color::Red,
        );
    } else if app.show_apply_popup {
        draw_popup(
            f,
            "⚡ Apply Snapshot ⚡",
            "Are you sure you want to rollback to this snapshot?\n\nSystem will need a reboot to take effect.\n\n[Enter] Confirm  [Esc] Cancel",
            Color::Yellow,
        );
    }
}

fn draw_popup(f: &mut Frame, title: &str, message: &str, border_color: Color) {
    let area = f.area();
    
    // Create centered popup area (60% width, 40% height)
    let popup_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area)[1];
    
    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(popup_area)[1];
    
    // Clear the popup area
    let clear = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(clear, popup_area);
    
    // Render popup border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .title(title)
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(Color::Black).fg(border_color));
    
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);
    
    // Render message
    let para = Paragraph::new(message)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White));
    
    let text_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(inner)[1];
    
    f.render_widget(para, text_area);
}

fn draw_loading_screen(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let spinner = app.spinner_frames[app.spinner_state];
    let text = vec![
        Line::from(Span::styled("Snapper TUI", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled(format!("Loading Snapshots... {}", spinner), Style::default().fg(Color::Yellow))),
    ];
    
    let block = Paragraph::new(text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    
    // Center the loading box
    let area = centered_rect(60, 20, area);
    f.render_widget(block, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_header(f: &mut Frame, area: Rect) {
    let text = Paragraph::new("Snapper TUI (Rust Port)")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(text, area);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Snapshot list
            Constraint::Percentage(50), // Right Panel (Details + Status)
        ])
        .split(area);

    draw_snapshot_table(f, app, chunks[0]);
    draw_right_panel(f, app, chunks[1]);
}

fn draw_right_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Details
            Constraint::Percentage(60), // Status
        ])
        .split(area);

    draw_details_panel(f, app, chunks[0]);
    draw_status_panel(f, app, chunks[1]);
}

fn draw_snapshot_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["#", "Type", "Date", "User", "Used Space", "Description"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Black).bg(Color::Cyan)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::Cyan))
        .height(1);

    let rows = app.snapshots.iter().map(|item| {
        let cells = vec![
            Cell::from(item.number.to_string()),
            Cell::from(item.snapshot_type.clone()),
            Cell::from(item.date.clone()),
            Cell::from(item.user.clone()),
            Cell::from(item.used_space.map(|s| s.to_string()).unwrap_or_default()),
            Cell::from(item.description.clone()),
        ];
        Row::new(cells).height(1)
    });

    let t = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title("Snapshots"))
    .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.table_state);
}

fn draw_details_panel(f: &mut Frame, app: &App, area: Rect) {
    let details_text = if let Some(snap) = app.get_selected_snapshot() {
        let userdata_str = snap.userdata.as_ref().map(|m| {
            m.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        }).unwrap_or_default();

        vec![
            Line::from(vec![Span::styled("Config: ", Style::default().fg(Color::Cyan)), Span::raw(&snap.config)]),
            Line::from(vec![Span::styled("Subvolume: ", Style::default().fg(Color::Cyan)), Span::raw(&snap.subvolume)]),
            Line::from(vec![Span::styled("Number: ", Style::default().fg(Color::Cyan)), Span::raw(snap.number.to_string())]),
            Line::from(vec![Span::styled("Type: ", Style::default().fg(Color::Cyan)), Span::raw(&snap.snapshot_type)]),
            Line::from(vec![Span::styled("Date: ", Style::default().fg(Color::Cyan)), Span::raw(&snap.date)]),
            Line::from(vec![Span::styled("User: ", Style::default().fg(Color::Cyan)), Span::raw(&snap.user)]),
            Line::from(vec![Span::styled("Cleanup: ", Style::default().fg(Color::Cyan)), Span::raw(snap.cleanup.as_deref().unwrap_or("-"))]),
            Line::from(vec![Span::styled("Description: ", Style::default().fg(Color::Cyan)), Span::raw(&snap.description)]),
            Line::from(vec![Span::styled("Used Space: ", Style::default().fg(Color::Cyan)), Span::raw(snap.used_space.map(|s| s.to_string()).unwrap_or_default())]),
            Line::from(vec![Span::styled("Userdata: ", Style::default().fg(Color::Cyan)), Span::raw(userdata_str)]),
        ]
    } else {
        vec![Line::from("Select a snapshot to view details.")]
    };

    let details = Paragraph::new(details_text)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title("Details"))
        .wrap(Wrap { trim: true })
        .scroll((app.details_scroll, 0));
    f.render_widget(details, area);
}

fn draw_status_panel(f: &mut Frame, app: &App, area: Rect) {
    let mut title = String::from("Status");
    if app.loading {
        title.push_str(&format!(" {}", app.spinner_frames[app.spinner_state]));
    }

    // Combine app.message (short status) and app.status_text (long output)
    let mut content = vec![
        Line::from(Span::styled(&app.message, Style::default().fg(if app.loading { Color::Yellow } else { Color::Green }))),
        Line::from(""),
    ];
    
    for line in app.status_text.lines() {
        content.push(Line::from(line));
    }

    let status = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(title))
        .wrap(Wrap { trim: true })
        .scroll((app.status_scroll, 0));
    f.render_widget(status, area);
}

fn draw_actions_bar(f: &mut Frame, area: Rect) {
    let actions_text = vec![
        Span::styled(" Actions: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("[D]elete ", Style::default().bg(Color::Red).fg(Color::White)),
        Span::raw(" "),
        Span::styled("[a]pply ", Style::default().bg(Color::Green).fg(Color::Black)),
        Span::raw(" "),
        Span::styled("[S]tatus ", Style::default().bg(Color::Blue).fg(Color::White)),
        Span::raw(" "),
        Span::styled("[r]efresh ", Style::default().bg(Color::Yellow).fg(Color::Black)),
        Span::raw(" "),
        Span::styled("[q]uit ", Style::default().bg(Color::Gray).fg(Color::Black)),
    ];
    
    let actions = Paragraph::new(Line::from(actions_text))
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded));
    f.render_widget(actions, area);
}
