use crate::app::{ActionButton, App};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
};
use snapper_core::format::format_size;
use snapper_core::query::SortKey;
use std::time::Instant;
use tachyonfx::{Duration, EffectRenderer, Interpolation, Shader, fx};

// Modern Color Palette (Cyberpunk / Dracula inspired)
const PALETTE_PRIMARY: Color = Color::Rgb(189, 147, 249);
const PALETTE_SECONDARY: Color = Color::Rgb(139, 233, 253);
const PALETTE_ACCENT: Color = Color::Rgb(255, 121, 198);
const PALETTE_SUCCESS: Color = Color::Rgb(80, 250, 123);
const PALETTE_WARNING: Color = Color::Rgb(241, 250, 140);
const PALETTE_ERROR: Color = Color::Rgb(255, 85, 85);
const PALETTE_BG_DARK: Color = Color::Rgb(30, 30, 46);
const PALETTE_FG: Color = Color::Rgb(248, 248, 242);
const PALETTE_GRAY: Color = Color::Rgb(98, 114, 164);
const PALETTE_BG_LIGHTER: Color = Color::Rgb(68, 71, 90);

const SLANT_RIGHT: &str = "";
const SLANT_LEFT: &str = "";

pub fn draw(f: &mut Frame, app: &mut App) {
    if app.show_splash
        && let Some(start) = app.splash_start
    {
        let elapsed_ms = start.elapsed().as_millis();
        if (!app.loading && elapsed_ms >= 500) || elapsed_ms >= 1500 {
            app.show_splash = false;
        } else {
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
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::ITALIC),
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(5),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(f.area());

        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(f.area());

        let header_area = intersection(chunks[1], main_layout[1]);
        let main_area = intersection(chunks[3], main_layout[1]);
        let footer_area = intersection(chunks[5], main_layout[1]);

        draw_header(f, app, header_area);
        draw_main(f, app, main_area);
        draw_actions_bar(f, app, footer_area);
    }

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

    if app.show_delete_popup {
        draw_delete_popup(f, app);
    }

    if app.show_create_popup {
        draw_create_popup(f, app);
    }

    if app.show_apply_popup {
        draw_apply_popup(f, app);
    }

    if app.loading {
        draw_loading_screen(f, app);
    }
}

fn draw_popup(f: &mut Frame, title: &str, message: &str, border_color: Color) {
    let area = f.area();

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

    f.render_widget(Clear, popup_area);

    let dark_bg = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(dark_bg, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .title(Span::styled(
            title,
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(popup_area);

    let inner_bg = Block::default().style(Style::default().bg(Color::Black));
    f.render_widget(inner_bg, inner);

    f.render_widget(block, popup_area);

    let para = Paragraph::new(message)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White).bg(Color::Black));

    let text_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(inner)[1];

    f.render_widget(para, text_area);
}

fn draw_delete_popup(f: &mut Frame, app: &mut App) {
    let targets = app.get_targets_for_delete();
    let count = targets.len().max(1);
    let protected_count = app.count_protected_delete_targets();

    let mut message = if count > 1 {
        format!(
            "Delete {} selected snapshots?\n\nThis action cannot be undone.",
            count
        )
    } else if let Some(target) = targets.first() {
        format!(
            "Delete snapshot {}?\n\nThis action cannot be undone.",
            target
        )
    } else {
        "Delete selected snapshot?\n\nThis action cannot be undone.".to_string()
    };

    if protected_count > 0 {
        message.push_str(&format!(
            "\n\n⚠️ Note: {} protected snapshot(s) (#0 / active / default) will be skipped.",
            protected_count
        ));
    }

    message.push_str("\n\n[Enter] Confirm  [Esc] Cancel");

    draw_popup(f, "🗑 DELETE SNAPSHOT 🗑", &message, PALETTE_ERROR);
}

fn draw_create_popup(f: &mut Frame, app: &mut App) {
    let area = centered_rect(60, 25, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(PALETTE_ACCENT))
        .title(Line::from(vec![
            Span::styled(
                " ➕ CREATE SNAPSHOT ",
                Style::default()
                    .fg(PALETTE_BG_DARK)
                    .bg(PALETTE_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                SLANT_RIGHT,
                Style::default().fg(PALETTE_ACCENT).bg(PALETTE_BG_DARK),
            ),
        ]))
        .title_alignment(Alignment::Left)
        .style(Style::default().bg(PALETTE_BG_DARK));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .margin(1)
        .split(inner_area);

    let target_cfg = app
        .get_selected_snapshot()
        .map(|s| s.config.as_str())
        .filter(|c| !c.is_empty())
        .unwrap_or("root");

    let prompt = Paragraph::new(format!(
        "Enter description for new '{}' snapshot:",
        target_cfg
    ))
    .style(Style::default().fg(PALETTE_FG))
    .alignment(Alignment::Center);
    f.render_widget(prompt, chunks[0]);

    let input = Paragraph::new(format!("{}█", app.create_input))
        .style(Style::default().fg(PALETTE_SECONDARY).bg(PALETTE_BG_LIGHTER))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(PALETTE_GRAY)),
        );
    f.render_widget(input, chunks[1]);

    let buttons = Paragraph::new(Line::from(vec![
        Span::styled(
            " [Enter] Create ",
            Style::default()
                .fg(PALETTE_SUCCESS)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            " [Esc] Cancel ",
            Style::default()
                .fg(PALETTE_ERROR)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(buttons, chunks[3]);
}

fn draw_apply_popup(f: &mut Frame, app: &mut App) {
    let target_label = app
        .get_target_for_apply()
        .map(|k| k.to_string())
        .unwrap_or_else(|| "selected snapshot".to_string());
    let message = format!(
        "Rollback to snapshot {}?\n\nSystem will need a reboot to take effect.\n\n[Enter] Confirm  [Esc] Cancel",
        target_label
    );
    draw_popup(f, "⚡ APPLY SNAPSHOT ⚡", &message, PALETTE_WARNING);
}

fn draw_loading_screen(f: &mut Frame, app: &mut App) {
    let spinner = app.spinner_frames[app.spinner_state];
    let text = vec![
        Line::from(Span::styled(
            "Snapper TUI",
            Style::default()
                .fg(PALETTE_SECONDARY)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{} {}", app.loading_message, spinner),
            Style::default().fg(PALETTE_WARNING),
        )),
    ];

    let block = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .style(Style::default().bg(PALETTE_BG_DARK)),
        );

    let area = centered_rect(60, 20, f.area());
    f.render_widget(Clear, area);
    f.render_widget(block, area);
}

fn intersection(r1: Rect, r2: Rect) -> Rect {
    let x = r1.x.max(r2.x);
    let y = r1.y.max(r2.y);
    let width = (r1.x + r1.width).min(r2.x + r2.width).saturating_sub(x);
    let height = (r1.y + r1.height).min(r2.y + r2.height).saturating_sub(y);
    Rect {
        x,
        y,
        width,
        height,
    }
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
                Span::styled(
                    "Filter: ",
                    Style::default()
                        .fg(PALETTE_SECONDARY)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    &app.filter_input,
                    Style::default().fg(PALETTE_FG).bg(PALETTE_BG_LIGHTER),
                ),
                Span::styled(
                    " █",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::SLOW_BLINK),
                ),
            ]),
            Line::from(""),
        ]
    } else if !app.filter_input.is_empty() {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "Filter: ",
                    Style::default()
                        .fg(PALETTE_SECONDARY)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&app.filter_input, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(""),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "  🔮 SNAPPER ",
                    Style::default()
                        .fg(PALETTE_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "TUI ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("⚡ ", Style::default().fg(PALETTE_WARNING)),
            ]),
            Line::from(vec![Span::styled(
                "  Cyberpunk Edition ",
                Style::default()
                    .fg(PALETTE_SECONDARY)
                    .add_modifier(Modifier::ITALIC),
            )]),
            Line::from(""),
        ]
    };

    let header = Paragraph::new(header_text)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(PALETTE_PRIMARY))
                .style(Style::default().bg(PALETTE_BG_DARK)),
        );
    f.render_widget(header, area);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    draw_snapshot_table(f, app, chunks[0]);
    draw_right_panel(f, app, chunks[2]);
}

fn draw_right_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(42),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    draw_details_panel(f, app, chunks[0]);
    draw_status_panel(f, app, chunks[2]);
}

fn draw_snapshot_table(f: &mut Frame, app: &mut App, area: Rect) {
    app.ui_areas.table = area;
    app.ui_areas.table_header_row = area.y.saturating_add(1);
    app.ui_areas.table_first_data_row = area.y.saturating_add(2);

    let header_cells = vec![
        Cell::from(format!("Cfg{}", app.get_sort_indicator(SortKey::Config))).style(
            Style::default()
                .fg(PALETTE_BG_DARK)
                .bg(PALETTE_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(format!("#{}", app.get_sort_indicator(SortKey::Number))).style(
            Style::default()
                .fg(PALETTE_BG_DARK)
                .bg(PALETTE_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(format!("Type{}", app.get_sort_indicator(SortKey::Type))).style(
            Style::default()
                .fg(PALETTE_BG_DARK)
                .bg(PALETTE_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(format!("Date{}", app.get_sort_indicator(SortKey::Date))).style(
            Style::default()
                .fg(PALETTE_BG_DARK)
                .bg(PALETTE_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(format!("User{}", app.get_sort_indicator(SortKey::User))).style(
            Style::default()
                .fg(PALETTE_BG_DARK)
                .bg(PALETTE_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(format!(
            "Space{}",
            app.get_sort_indicator(SortKey::UsedSpace)
        ))
        .style(
            Style::default()
                .fg(PALETTE_BG_DARK)
                .bg(PALETTE_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from(format!(
            "Description{}",
            app.get_sort_indicator(SortKey::Description)
        ))
        .style(
            Style::default()
                .fg(PALETTE_BG_DARK)
                .bg(PALETTE_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    let header = Row::new(header_cells)
        .style(Style::default().bg(PALETTE_PRIMARY))
        .height(1);

    let mut table_state = std::mem::take(&mut app.table_state);
    let snapshots = app.get_filtered_snapshots();

    let rows: Vec<Row> = snapshots
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = app.selected_keys.contains(&item.key());
            let marker = if is_selected {
                "✅ "
            } else if item.active {
                "● "
            } else {
                ""
            };

            let cells = vec![
                Cell::from(item.config.as_str()),
                Cell::from(format!("{}{}", marker, item.number)),
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
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Length(10),
            Constraint::Length(10),
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
                Span::styled(
                    " 📦 SNAPSHOTS ",
                    Style::default()
                        .fg(PALETTE_BG_DARK)
                        .bg(PALETTE_SECONDARY)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    SLANT_RIGHT,
                    Style::default().fg(PALETTE_SECONDARY).bg(PALETTE_BG_DARK),
                ),
            ]))
            .title_alignment(Alignment::Left)
            .style(Style::default().bg(PALETTE_BG_DARK)),
    )
    .highlight_style(
        Style::default()
            .bg(PALETTE_ACCENT)
            .fg(PALETTE_BG_DARK)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("👉 ");

    f.render_stateful_widget(t, area, &mut table_state);
    app.table_state = table_state;
}

fn draw_details_panel(f: &mut Frame, app: &mut App, area: Rect) {
    app.ui_areas.details = area;
    let selected = app.get_selected_snapshot();

    let content = if let Some(snap) = selected {
        let userdata_str = snap
            .userdata
            .as_ref()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let used_space_str = snap
            .used_space
            .map(|s| format!("{} ({} B)", format_size(s), s))
            .unwrap_or_else(|| String::from("-"));

        vec![
            Line::from(vec![
                Span::styled(
                    "⚙️ Config: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&snap.config, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled(
                    "📂 Subvolume: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&snap.subvolume, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled(
                    "🔢 Number: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(snap.number.to_string(), Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled(
                    "🏷️ Type: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&snap.snapshot_type, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled(
                    "📅 Date: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&snap.date, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled(
                    "👤 User: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&snap.user, Style::default().fg(PALETTE_SUCCESS)),
            ]),
            Line::from(vec![
                Span::styled(
                    "🧹 Cleanup: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    snap.cleanup.as_deref().unwrap_or("-"),
                    Style::default().fg(PALETTE_FG),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "📝 Description: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&snap.description, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled(
                    "💾 Used Space: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(used_space_str, Style::default().fg(PALETTE_FG)),
            ]),
            Line::from(vec![
                Span::styled(
                    "📋 Userdata: ",
                    Style::default()
                        .fg(PALETTE_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(userdata_str, Style::default().fg(PALETTE_FG)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "No snapshot selected.",
            Style::default()
                .fg(PALETTE_GRAY)
                .add_modifier(Modifier::ITALIC),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(PALETTE_ACCENT))
        .title(Line::from(vec![
            Span::styled(
                " 🔍 DETAILS ",
                Style::default()
                    .fg(PALETTE_BG_DARK)
                    .bg(PALETTE_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                SLANT_RIGHT,
                Style::default().fg(PALETTE_ACCENT).bg(PALETTE_BG_DARK),
            ),
        ]))
        .title_alignment(Alignment::Left)
        .style(Style::default().bg(PALETTE_BG_DARK));

    let inner = block.inner(area);
    let total_wrapped = wrapped_line_count(&content, inner.width);
    let max_scroll = total_wrapped.saturating_sub(inner.height);
    let scroll_y = app.details_scroll.min(max_scroll);

    let para = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((scroll_y, 0));

    f.render_widget(para, area);
    app.ui_areas.details_max_scroll = max_scroll;
    app.details_scroll = scroll_y;
}

fn wrapped_line_count(lines: &[Line<'_>], width: u16) -> u16 {
    let w = width.max(1) as usize;
    lines
        .iter()
        .map(|l| l.width().max(1).div_ceil(w) as u16)
        .fold(0u16, u16::saturating_add)
}

fn draw_status_panel(f: &mut Frame, app: &mut App, area: Rect) {
    app.ui_areas.status = area;
    let mut title = String::from(" ℹ️ STATUS ");
    if app.loading {
        title.push_str(&format!(" {}", app.spinner_frames[app.spinner_state]));
    }

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            &app.message,
            Style::default().fg(if app.loading {
                PALETTE_WARNING
            } else {
                PALETTE_SUCCESS
            }),
        )),
        Line::from(""),
    ];

    for line in app.status_text.lines() {
        lines.push(Line::from(Span::styled(
            line,
            Style::default().fg(PALETTE_FG),
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(PALETTE_WARNING))
        .title(Line::from(vec![
            Span::styled(
                title,
                Style::default()
                    .fg(PALETTE_BG_DARK)
                    .bg(PALETTE_WARNING)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                SLANT_RIGHT,
                Style::default().fg(PALETTE_WARNING).bg(PALETTE_BG_DARK),
            ),
        ]))
        .title_alignment(Alignment::Left)
        .style(Style::default().bg(PALETTE_BG_DARK));

    let inner = block.inner(area);
    let total_wrapped = wrapped_line_count(&lines, inner.width);
    app.ui_areas.status_max_scroll = total_wrapped.saturating_sub(inner.height);
    if app.status_scroll > app.ui_areas.status_max_scroll {
        app.status_scroll = app.ui_areas.status_max_scroll;
    }

    let status = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true })
        .scroll((app.status_scroll, 0));
    f.render_widget(status, area);
}

fn draw_actions_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let prefix = Span::styled(
        " ⚡ ACTIONS: ",
        Style::default()
            .fg(PALETTE_PRIMARY)
            .add_modifier(Modifier::BOLD),
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

    app.ui_areas.footer_buttons.clear();
    let total_width = line.width() as u16;
    let mut cursor_x = inner
        .x
        .saturating_add(inner.width.saturating_sub(total_width) / 2);
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
        cursor_x = cursor_x.saturating_add(btn_width + 1);
    }

    let actions = Paragraph::new(line)
        .alignment(Alignment::Center)
        .block(block);
    f.render_widget(actions, area);
}
