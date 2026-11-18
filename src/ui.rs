use crate::app::{format_size, ActionButton, App, SortKey};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};
use std::time::Instant;
use tachyonfx::{fx, Duration, EffectRenderer, Interpolation, Shader};

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

const SLANT_RIGHT: &str = "";
const SLANT_LEFT: &str = "";

pub fn draw(f: &mut Frame, app: &mut App) {
    // Splash Screen - simple custom implementation
    if app.show_splash
        && let Some(start) = app.splash_start
    {
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
                        "▄▄█ █░▀█ █▀█ █▀▀ █▀▀ ██▄ █▀▄",
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

    // Initialize effect if not present
    if app.fx.is_none() && app.fx_last_tick.is_none() && !app.show_splash {
        let effect = fx::fade_from(
            ratatui::style::Color::Black,
            ratatui::style::Color::Black,
            (Duration::from_millis(800), Interpolation::Linear),
        );
        app.fx = Some(effect);
        app.fx_last_tick = Some(Instant::now());
    }

    if !app.snapshots.is_empty() || !app.loading {
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
        let header_area = intersection(chunks[1], main_layout[1]);
        let main_area = intersection(chunks[3], main_layout[1]);
        let footer_area = intersection(chunks[5], main_layout[1]);

        draw_header(f, app, header_area);
        draw_main(f, app, main_area);
        draw_actions_bar(f, app, footer_area);
    }

    // Render TachyonFX effects using frame delta time and clear once complete
    if let Some(effect) = &mut app.fx {
        let now = Instant::now();
        let delta = app
            .fx_last_tick
            .map(|prev| now.duration_since(prev))
            .unwrap_or_default();
        app.fx_last_tick = Some(now);
        f.render_effect(effect, f.area(), delta.into());
        if effect.done() {
            app.fx = None;
        }
    }

    // Custom Popups - render on top
    if app.show_delete_popup {
        draw_delete_popup(f, app);
    }
    
    if app.show_create_popup {
        draw_create_popup(f, app);
    }
    
    if app.show_apply_popup {
        draw_apply_popup(f, app);
    }

    // Overlay Loading Screen if loading (Render last to be on top)
    if app.loading {
        draw_loading_screen(f, app);
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

fn draw_delete_popup(f: &mut Frame, app: &mut App) {
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
}

fn draw_create_popup(f: &mut Frame, app: &mut App) {
    let area = centered_rect(60, 25, f.area());
    
    // Clear area
    f.render_widget(Clear, area);
    
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(PALETTE_ACCENT))
        .title(Line::from(vec![
            Span::styled(" ➕ CREATE SNAPSHOT ", Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(SLANT_RIGHT, Style::default().fg(PALETTE_ACCENT).bg(PALETTE_BG_DARK)),
        ]))
        .title_alignment(Alignment::Left)
        .style(Style::default().bg(PALETTE_BG_DARK));
        
    let inner_area = block.inner(area);
    f.render_widget(block, area);
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Prompt
            Constraint::Length(3), // Input
            Constraint::Min(1),    // Gap
            Constraint::Length(3), // Buttons
        ])
        .margin(1)
        .split(inner_area);
        
    let prompt = Paragraph::new("Enter description for the new snapshot:")
        .style(Style::default().fg(PALETTE_FG))
        .alignment(Alignment::Center);
    f.render_widget(prompt, chunks[0]);
    
    let input = Paragraph::new(format!("{}█", app.create_input))
        .style(Style::default().fg(PALETTE_SECONDARY).bg(PALETTE_BG_LIGHTER))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(PALETTE_GRAY)));
    f.render_widget(input, chunks[1]);
    
    let buttons = Paragraph::new(Line::from(vec![
        Span::styled(" [Enter] Create ", Style::default().fg(PALETTE_SUCCESS).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled(" [Esc] Cancel ", Style::default().fg(PALETTE_ERROR).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(buttons, chunks[3]);
}

fn draw_apply_popup(f: &mut Frame, _app: &mut App) {
    draw_popup(
        f,
        "⚡ APPLY SNAPSHOT ⚡",
        "Are you sure you want to rollback to this snapshot?\n\nSystem will need a reboot to take effect.\n\n[Enter] Confirm  [Esc] Cancel",
        PALETTE_WARNING,
    );
}

fn draw_loading_screen(f: &mut Frame, app: &mut App) {
    let spinner = app.spinner_frames[app.spinner_state];
    let text = vec![
        Line::from(Span::styled("Snapper TUI", Style::default().fg(PALETTE_SECONDARY).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled(format!("{} {}", app.loading_message, spinner), Style::default().fg(PALETTE_WARNING))),
    ];
    
    let block = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).style(Style::default().bg(PALETTE_BG_DARK)));
    
    // Center the loading box
    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area); // Clear background
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

fn draw_header(f: &mut Frame, app: &mut App, area: Rect) {
    let header_text = if app.filtering {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Filter: ", Style::default().fg(PALETTE_SECONDARY).add_modifier(Modifier::BOLD)),
                Span::styled(&app.filter_input, Style::default().fg(PALETTE_FG).bg(PALETTE_BG_LIGHTER)),
                Span::styled(" █", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::SLOW_BLINK)),
            ]),
            Line::from(""),
        ]
    } else if !app.filter_input.is_empty() {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Filter: ", Style::default().fg(PALETTE_SECONDARY).add_modifier(Modifier::BOLD)),
                Span::styled(&app.filter_input, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(""),
        ]
    } else {
        vec![
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
        ]
    };

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
    app.ui_areas.table = area;
    app.ui_areas.table_header_row = area.y.saturating_add(1);
    app.ui_areas.table_first_data_row = area.y.saturating_add(2);

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

    let mut table_state = std::mem::take(&mut app.table_state);
    let snapshots = app.get_filtered_snapshots();

    // Zebra striping with modern colors
    let rows: Vec<Row> = snapshots
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = app
                .selected_keys
                .iter()
                .any(|(cfg, num)| *num == item.number && cfg == &item.config);
            let selection_marker = if is_selected { "✅ " } else { "" };

            let cells = vec![
                Cell::from(format!("{}{}", selection_marker, item.number)),
                Cell::from(item.snapshot_type.as_str()),
                Cell::from(item.date.as_str()),
                Cell::from(item.user.as_str()),
                Cell::from(item.used_space.map(format_size).unwrap_or_default()),
                Cell::from(item.description.as_str()),
            ];
            let bg = if idx % 2 == 0 {
                PALETTE_BG_DARK
            } else {
                PALETTE_BG_LIGHTER
            };
            Row::new(cells)
                .height(1)
                .style(Style::default().bg(bg).fg(PALETTE_FG))
        })
        .collect();

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
                .title(Line::from(vec![
                    Span::styled(" 📦 SNAPSHOTS ", Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_SECONDARY).add_modifier(Modifier::BOLD)),
                    Span::styled(SLANT_RIGHT, Style::default().fg(PALETTE_SECONDARY).bg(PALETTE_BG_DARK)),
                ]))
                .title_alignment(Alignment::Left)
                .style(Style::default().bg(PALETTE_BG_DARK))
        )
        .highlight_style(Style::default().bg(PALETTE_ACCENT).fg(PALETTE_BG_DARK).add_modifier(Modifier::BOLD))
        .highlight_symbol("👉 ");

    f.render_stateful_widget(t, area, &mut table_state);
    app.table_state = table_state;
}

fn draw_details_panel(f: &mut Frame, app: &mut App, area: Rect) {
    app.ui_areas.details = area;
    let selected = app.get_selected_snapshot();

    let content = if let Some(snap) = selected {
        let userdata_str = snap.userdata.as_ref().map(|m| {
            m.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        }).unwrap_or_default();

        let used_space_str = snap
            .used_space
            .map(|s| format!("{} ({} B)", format_size(s), s))
            .unwrap_or_else(|| String::from("-"));

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
                Span::styled(used_space_str, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled("📋 Userdata: ", Style::default().fg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(userdata_str, Style::default().fg(PALETTE_FG)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled("No snapshot selected.", Style::default().fg(PALETTE_GRAY).add_modifier(Modifier::ITALIC)))]
    };

    let inner_height = area.height.saturating_sub(2);
    let max_scroll = (content.len() as u16).saturating_sub(inner_height);
    let scroll_y = app.details_scroll.min(max_scroll);

    let para = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(PALETTE_ACCENT))
                .title(Line::from(vec![
                    Span::styled(" 🔍 DETAILS ", Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_ACCENT).add_modifier(Modifier::BOLD)),
                    Span::styled(SLANT_RIGHT, Style::default().fg(PALETTE_ACCENT).bg(PALETTE_BG_DARK)),
                ]))
                .title_alignment(Alignment::Left)
                .style(Style::default().bg(PALETTE_BG_DARK))
        )
        .wrap(Wrap { trim: true })
        .scroll((scroll_y, 0));

    f.render_widget(para, area);
    app.ui_areas.details_max_scroll = max_scroll;
    app.details_scroll = scroll_y;
}

fn draw_status_panel(f: &mut Frame, app: &mut App, area: Rect) {
    app.ui_areas.status = area;
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

    let inner_height = area.height.saturating_sub(2);
    app.ui_areas.status_max_scroll = (lines.len() as u16).saturating_sub(inner_height);
    if app.status_scroll > app.ui_areas.status_max_scroll {
        app.status_scroll = app.ui_areas.status_max_scroll;
    }

    let status = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(PALETTE_WARNING))
                .title(Line::from(vec![
                    Span::styled(title, Style::default().fg(PALETTE_BG_DARK).bg(PALETTE_WARNING).add_modifier(Modifier::BOLD)),
                    Span::styled(SLANT_RIGHT, Style::default().fg(PALETTE_WARNING).bg(PALETTE_BG_DARK)),
                ]))
                .title_alignment(Alignment::Left)
                .style(Style::default().bg(PALETTE_BG_DARK))
        )
        .wrap(Wrap { trim: true })
        .scroll((app.status_scroll, 0));
    f.render_widget(status, area);
}

fn draw_actions_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let prefix = Span::styled(
        " ⚡ ACTIONS: ",
        Style::default().fg(PALETTE_PRIMARY).add_modifier(Modifier::BOLD),
    );

    let buttons: [(&str, Color, ActionButton); 7] = [
        (" [C]reate ➕ ", PALETTE_ACCENT, ActionButton::Create),
        (" [D]elete 🗑️  ", PALETTE_ERROR, ActionButton::Delete),
        (" [A]pply ↩️  ", PALETTE_SUCCESS, ActionButton::Apply),
        (" [/] Filter 🔍 ", PALETTE_PRIMARY, ActionButton::Filter),
        (" [S]tatus ℹ️  ", PALETTE_SECONDARY, ActionButton::Status),
        (" [R]efresh 🔄 ", PALETTE_WARNING, ActionButton::Refresh),
        (" [Q]uit 🚪 ", PALETTE_GRAY, ActionButton::Quit),
    ];

    let mut actions_text = vec![prefix.clone()];
    let mut button_widths: Vec<(u16, ActionButton)> = Vec::with_capacity(buttons.len());

    for (label, color, action) in buttons {
        let s1 = Span::styled(SLANT_LEFT, Style::default().fg(color).bg(PALETTE_BG_DARK));
        let s2 = Span::styled(
            label,
            Style::default()
                .bg(color)
                .fg(PALETTE_BG_DARK)
                .add_modifier(Modifier::BOLD),
        );
        let s3 = Span::styled(SLANT_LEFT, Style::default().fg(PALETTE_BG_DARK).bg(color));
        let btn_width = (s1.width() + s2.width() + s3.width()) as u16;
        button_widths.push((btn_width, action));
        actions_text.push(s1);
        actions_text.push(s2);
        actions_text.push(s3);
        actions_text.push(Span::raw(" "));
    }

    let line = Line::from(actions_text);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(PALETTE_GRAY))
        .style(Style::default().bg(PALETTE_BG_DARK));
    let inner = block.inner(area);

    // Record exact hit-test rects matching Alignment::Center
    app.ui_areas.footer_buttons.clear();
    let total_width = line.width() as u16;
    let mut cursor_x = inner.x.saturating_add(inner.width.saturating_sub(total_width) / 2);
    cursor_x = cursor_x.saturating_add(prefix.width() as u16);
    for (btn_width, action) in button_widths {
        app.ui_areas.footer_buttons.push((
            Rect {
                x: cursor_x,
                y: inner.y,
                width: btn_width,
                height: inner.height,
            },
            action,
        ));
        cursor_x = cursor_x.saturating_add(btn_width + 1); // +1 for trailing space
    }

    let actions = Paragraph::new(line)
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(actions, area);
}
