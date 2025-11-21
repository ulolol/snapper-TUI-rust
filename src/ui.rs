use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Cell, Paragraph, Row, Table, Wrap, Clear},
    Frame,
};
use tachyonfx::{
    fx, Duration, EffectRenderer, Interpolation,
};

// Modern Color Palette (GitHub Dark / Dracula inspired)
// Modern Color Palette (Cyberpunk / Dracula inspired)
const PALETTE_PRIMARY: Color = Color::Rgb(189, 147, 249);    // Deep Purple
const PALETTE_SECONDARY: Color = Color::Rgb(139, 233, 253);  // Cyan
const PALETTE_ACCENT: Color = Color::Rgb(255, 121, 198);     // Pink
const PALETTE_SUCCESS: Color = Color::Rgb(80, 250, 123);     // Green
const PALETTE_WARNING: Color = Color::Rgb(241, 250, 140);    // Yellow
const PALETTE_ERROR: Color = Color::Rgb(255, 85, 85);        // Red
const PALETTE_BG_DARK: Color = Color::Rgb(30, 30, 46);       // Darker Background
const PALETTE_FG: Color = Color::Rgb(248, 248, 242);         // Foreground
const PALETTE_GRAY: Color = Color::Rgb(98, 114, 164);        // Gray
const PALETTE_BG_LIGHTER: Color = Color::Rgb(68, 71, 90);    // Lighter Background

pub fn draw(f: &mut Frame, app: &mut App) {
    // Splash Screen - simple custom implementation
    if app.show_splash {
        if let Some(start) = app.splash_start {
            if start.elapsed().as_secs() >= 2 {
                app.show_splash = false;
            } else {
                // Render simple centered splash with gradient colors
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(PALETTE_PRIMARY))
                    .style(Style::default().bg(Color::Black));
                f.render_widget(block, f.area());
                
                let text = vec![
                    Line::from(""),
                    Line::from(""),
                    Line::from(""),
                    Line::from(Span::styled(
                        "█▀▀ █▄░█ █▀█ █▀█ █▀█ █▀▀ █▀█",
                        Style::default()
                            .fg(PALETTE_PRIMARY)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        "▄█  █░▀█ █▀█ █▀▀ █▀▀ ██▄ █▀▄",
                        Style::default()
                            .fg(PALETTE_SECONDARY)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        "              TUI",
                        Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::ITALIC),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "⚡ Initializing System...",
                        Style::default().fg(PALETTE_WARNING),
                    )),
                ];
                
                let para = Paragraph::new(text).alignment(Alignment::Center);
                let center = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(35),
                        Constraint::Length(9),
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
        // Create a "floating" layout with gaps
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Top Gap
                Constraint::Length(5), // Header
                Constraint::Length(1), // Gap
                Constraint::Min(0),    // Main
                Constraint::Length(1), // Gap
                Constraint::Length(3), // Footer
                Constraint::Length(1), // Bottom Gap
            ])
            .split(f.area());

        // Add horizontal padding
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // Left Gap
                Constraint::Min(0),    // Content
                Constraint::Length(2), // Right Gap
            ])
            .split(f.area());
        
        // Intersect vertical chunks with horizontal padding
        // We'll pass the specific areas to the draw functions
        
        // Helper to intersect rects (simple version for this layout)
        let header_area = intersection(chunks[1], main_layout[1]);
        let main_area = intersection(chunks[3], main_layout[1]);
        let footer_area = intersection(chunks[5], main_layout[1]);

        draw_header(f, header_area);
        draw_main(f, app, main_area);
        draw_actions_bar(f, footer_area);
    }

    // Render TachyonFX effects
    if let Some(effect) = &mut app.fx {
        if let Some(start) = app.fx_start {
            f.render_effect(effect, f.area(), start.elapsed().into());
        }
    }

    // Custom Popups - render on top
    if app.show_delete_popup {
        let count = if app.get_selected_count() > 0 {
            app.get_selected_count()
        } else {
            1
        };
        
        let message = if count > 1 {
            format!("Delete {} selected snapshots?\n\nThis action cannot be undone.\n\n[Enter] Confirm  [Esc] Cancel", count)
        } else {
            "Delete selected snapshot?\n\nThis action cannot be undone.\n\n[Enter] Confirm  [Esc] Cancel".to_string()
        };
        
        draw_popup(
            f,
            "🗑 DELETE SNAPSHOT 🗑",
            &message,
            PALETTE_ERROR,
        );
    } else if app.show_apply_popup {
        draw_popup(
            f,
            "⚡ APPLY SNAPSHOT ⚡",
            "Are you sure you want to rollback to this snapshot?\n\nSystem will need a reboot to take effect.\n\n[Enter] Confirm  [Esc] Cancel",
            PALETTE_WARNING,
        );
    }
}

fn draw_popup(f: &mut Frame, title: &str, message: &str, border_color: Color) {
    let area = f.area();
    
    // Create centered popup area (65% width, 45% height for better readability)
    let popup_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(28),
            Constraint::Percentage(44),
            Constraint::Percentage(28),
        ])
        .split(area)[1];
    
    let popup_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(17),
            Constraint::Percentage(66),
            Constraint::Percentage(17),
        ])
        .split(popup_area)[1];
    
    // CRITICAL: Use Clear widget to make popup opaque
    // This clears the area so background doesn't bleed through
    f.render_widget(Clear, popup_area);
    
    // Render fully opaque black background for legibility
    let dark_bg = Block::default()
        .style(Style::default().bg(Color::Black));
    f.render_widget(dark_bg, popup_area);
    
    // Render popup border with modern double-line style
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .title(Span::styled(title, Style::default().fg(border_color).add_modifier(Modifier::BOLD)))
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Black));
    
    let inner = block.inner(popup_area);
    
    // Fill inner area with black background too
    let inner_bg = Block::default()
        .style(Style::default().bg(Color::Black));
    f.render_widget(inner_bg, inner);
    
    f.render_widget(block, popup_area);
    
    // Render message with bright white text for maximum contrast
    let para = Paragraph::new(message)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White).bg(Color::Black));
    
    // Center the text vertically within the popup
    let text_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(15), Constraint::Percentage(70), Constraint::Percentage(15)])
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

fn intersection(r1: Rect, r2: Rect) -> Rect {
    let x = r1.x.max(r2.x);
    let y = r1.y.max(r2.y);
    let width = (r1.x + r1.width).min(r2.x + r2.width).saturating_sub(x);
    let height = (r1.y + r1.height).min(r2.y + r2.height).saturating_sub(y);
    Rect { x, y, width, height }
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
    // Custom header with large styled text
    let header_text = vec![
        Line::from(""), // Empty line for spacing
        Line::from(vec![
            Span::styled("  🔮 SNAPPER ", Style::default().fg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled("TUI ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled("⚡ ", Style::default().fg(PALETTE_WARNING)),
        ]),
        Line::from(vec![
            Span::styled("  Cyberpunk Edition ", Style::default().fg(PALETTE_SECONDARY).add_modifier(Modifier::ITALIC)),
        ]),
        Line::from(""), // Empty line for spacing
    ];

    let header = Paragraph::new(header_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(PALETTE_PRIMARY))
                .style(Style::default().bg(PALETTE_BG_DARK))
        );

    f.render_widget(header, area);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Snapshot list
            Constraint::Length(1),      // Gap
            Constraint::Min(0),         // Right Panel (Details + Status)
        ])
        .split(area);

    draw_snapshot_table(f, app, chunks[0]);
    // chunks[1] is gap
    draw_right_panel(f, app, chunks[2]);
}

fn draw_right_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Details
            Constraint::Length(1),      // Gap
            Constraint::Min(0),         // Status
        ])
        .split(area);

    draw_details_panel(f, app, chunks[0]);
    // chunks[1] is gap
    draw_status_panel(f, app, chunks[2]);
}

fn draw_snapshot_table(f: &mut Frame, app: &mut App, area: Rect) {
    use crate::app::{format_size, SortKey};
    
    // Modern header with primary color and sort indicators
    let header_cells = vec![
        Cell::from(format!("📸 #{}", app.get_sort_indicator(SortKey::Number)))
            .style(Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD)),
        Cell::from(format!("🏷️ Type{}", app.get_sort_indicator(SortKey::Type)))
            .style(Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD)),
        Cell::from(format!("📅 Date{}", app.get_sort_indicator(SortKey::Date)))
            .style(Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD)),
        Cell::from(format!("👤 User{}", app.get_sort_indicator(SortKey::User)))
            .style(Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD)),
        Cell::from(format!("💾 Space{}", app.get_sort_indicator(SortKey::UsedSpace)))
            .style(Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD)),
        Cell::from("📝 Description")
            .style(Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD)),
    ];
    let header = Row::new(header_cells)
        .style(Style::default().bg(PALETTE_PRIMARY))
        .height(1);

    // Zebra striping with modern colors
    let rows = app.snapshots.iter().enumerate().map(|(idx, item)| {
        let is_selected = app.selected_indices.contains(&idx);
        let selection_marker = if is_selected { "✅ " } else { "" };
        
        let cells = vec![
            Cell::from(format!("{}{}", selection_marker, item.number)),
            Cell::from(item.snapshot_type.clone()),
            Cell::from(item.date.clone()),
            Cell::from(item.user.clone()),
            Cell::from(item.used_space.map(|s| format_size(s)).unwrap_or_default()),
            Cell::from(item.description.clone()),
        ];
        // Zebra striping
        let bg = if idx % 2 == 0 { PALETTE_BG_DARK } else { PALETTE_BG_LIGHTER };
        Row::new(cells).height(1).style(Style::default().bg(bg).fg(PALETTE_FG))
    });

    let t = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(22),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Min(10),
        ],
    )
    .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(PALETTE_SECONDARY))
                .title(Span::styled(" 📦 SNAPSHOTS ", Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_SECONDARY).add_modifier(Modifier::BOLD)))
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(PALETTE_BG_DARK))
        )
        .highlight_style(Style::default().bg(PALETTE_ACCENT).fg(PALETTE_BG_DARK).add_modifier(Modifier::BOLD))
        .highlight_symbol("👉 ");

    f.render_stateful_widget(t, area, &mut app.table_state);
}

fn draw_details_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let selected = app.get_selected_snapshot();

    let content = if let Some(snap) = selected {
        let userdata_str = snap.userdata.as_ref().map(|m| {
            m.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        }).unwrap_or_default();

        vec![
            Line::from(vec![
                Span::styled("⚙️ Config: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(&snap.config, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("📂 Subvolume: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(&snap.subvolume, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("🔢 Number: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(snap.number.to_string(), Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("🏷️ Type: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(&snap.snapshot_type, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("📅 Date: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(&snap.date, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("👤 User: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(&snap.user, Style::default().fg(PALETTE_SUCCESS)),
            ]),
            Line::from(vec![
                Span::styled("🧹 Cleanup: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(snap.cleanup.as_deref().unwrap_or("-"), Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("📝 Description: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(&snap.description, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("💾 Used Space: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(snap.used_space.map(|s| s.to_string()).unwrap_or_default(), Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("📋 Userdata: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(userdata_str, Style::default().fg(PALETTE_FG)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled("No snapshot selected.", Style::default().fg(PALETTE_GRAY).add_modifier(Modifier::ITALIC)))]
    };

    let para = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(PALETTE_ACCENT))
                .title(Span::styled(" 🔍 DETAILS ", Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)))
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(PALETTE_BG_DARK))
        )
        .wrap(Wrap { trim: true })
        .scroll((app.details_scroll as u16, 0));

    f.render_widget(para, area);
}

fn draw_status_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let mut title = String::from(" ℹ️ STATUS ");
    if app.loading {
        title.push_str(&format!(" {}", app.spinner_frames[app.spinner_state]));
    }

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(&app.message, Style::default().fg(if app.loading { PALETTE_WARNING } else { PALETTE_SUCCESS }))),
        Line::from(""),
    ];
    
    for line in app.status_text.lines() {
        lines.push(Line::from(Span::styled(line, Style::default().fg(PALETTE_FG))));
    }

    let status = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(PALETTE_WARNING))
                .title(Span::styled(title, Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_WARNING).add_modifier(Modifier::BOLD)))
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(PALETTE_BG_DARK))
        )
        .wrap(Wrap { trim: true })
        .scroll((app.status_scroll as u16, 0));
    f.render_widget(status, area);
}

fn draw_actions_bar(f: &mut Frame, area: Rect) {
    let actions_text = vec![
        Span::styled(" ⚡ ACTIONS: ", Style::default().fg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD)),
        Span::styled(" [D]elete 🗑️  ", Style::default().bg(PALETTE_ERROR).fg(PALETTE_BG_DARK).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(" [A]pply ↩️  ", Style::default().bg(PALETTE_SUCCESS).fg(PALETTE_BG_DARK).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(" [S]tatus ℹ️  ", Style::default().bg(PALETTE_SECONDARY).fg(PALETTE_BG_DARK).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(" [R]efresh 🔄 ", Style::default().bg(PALETTE_WARNING).fg(PALETTE_BG_DARK).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(" [Q]uit 🚪 ", Style::default().bg(PALETTE_GRAY).fg(PALETTE_BG_DARK).add_modifier(Modifier::BOLD)),
    ];
    
    let actions = Paragraph::new(Line::from(actions_text))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Double).border_style(Style::default().fg(PALETTE_GRAY)).style(Style::default().bg(PALETTE_BG_DARK)));
    f.render_widget(actions, area);
}
