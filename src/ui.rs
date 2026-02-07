use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{container_name, container_ports, App, Mode, Tab};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),   // content
            Constraint::Length(3), // status bar
        ])
        .split(f.area());

    draw_tabs(f, app, chunks[0]);

    match app.mode {
        Mode::Logs => draw_logs(f, app, chunks[1]),
        _ => match app.tab {
            Tab::Containers => draw_containers(f, app, chunks[1]),
            Tab::Networks => draw_networks(f, app, chunks[1]),
        },
    }

    draw_status_bar(f, app, chunks[2]);
}

// ── Tab Bar ────────────────────────────────────────────────────────────────

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = [Tab::Containers, Tab::Networks]
        .iter()
        .map(|t| {
            let style = if *t == app.tab {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(t.title(), style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" dockerrs "))
        .highlight_style(Style::default().fg(Color::Cyan))
        .select(match app.tab {
            Tab::Containers => 0,
            Tab::Networks => 1,
        })
        .divider(Span::raw(" | "));

    f.render_widget(tabs, area);
}

// ── Containers ─────────────────────────────────────────────────────────────

fn draw_containers(f: &mut Frame, app: &mut App, area: Rect) {
    if app.mode == Mode::Detail {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        draw_container_table(f, app, chunks[0]);
        draw_container_detail(f, app, chunks[1]);
    } else {
        draw_container_table(f, app, area);
    }
}

fn draw_container_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Status"),
        Cell::from("Image"),
        Cell::from("Ports"),
        Cell::from("ID"),
    ])
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = app
        .containers
        .iter()
        .map(|c| {
            let state = c.state.as_deref().unwrap_or("unknown");
            let color = state_color(state);

            Row::new(vec![
                Cell::from(container_name(c)),
                Cell::from(c.status.clone().unwrap_or_default()).style(Style::default().fg(color)),
                Cell::from(c.image.clone().unwrap_or_default()),
                Cell::from(container_ports(c)),
                Cell::from(
                    c.id.as_deref()
                        .map(|id| if id.len() > 12 { &id[..12] } else { id })
                        .unwrap_or("")
                        .to_string(),
                ),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Containers "),
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    f.render_stateful_widget(table, area, &mut app.container_table_state);
}

fn draw_container_detail(f: &mut Frame, app: &App, area: Rect) {
    let Some(c) = app.selected_container() else {
        let p = Paragraph::new("No container selected")
            .block(Block::default().borders(Borders::ALL).title(" Detail "));
        f.render_widget(p, area);
        return;
    };

    let id = c.id.as_deref().unwrap_or("N/A");
    let image = c.image.as_deref().unwrap_or("N/A");
    let image_id = c.image_id.as_deref().unwrap_or("N/A");
    let command = c.command.as_deref().unwrap_or("N/A");
    let state = c.state.as_deref().unwrap_or("N/A");
    let status = c.status.as_deref().unwrap_or("N/A");
    let ports = container_ports(c);

    let labels = c
        .labels
        .as_ref()
        .map(|l| {
            l.iter()
                .map(|(k, v)| format!("  {}={}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "  (none)".into());

    let networks = c
        .network_settings
        .as_ref()
        .and_then(|ns| ns.networks.as_ref())
        .map(|nets| {
            nets.keys()
                .map(|k| format!("  {}", k))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "  (none)".into());

    let mounts = c
        .mounts
        .as_ref()
        .map(|m| {
            m.iter()
                .map(|mount| {
                    format!(
                        "  {} -> {}",
                        mount.source.as_deref().unwrap_or("?"),
                        mount.destination.as_deref().unwrap_or("?")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "  (none)".into());

    let text = format!(
        "ID:       {}\n\
         Image:    {}\n\
         ImageID:  {}\n\
         Command:  {}\n\
         State:    {}\n\
         Status:   {}\n\
         Ports:    {}\n\n\
         Labels:\n{}\n\n\
         Networks:\n{}\n\n\
         Mounts:\n{}",
        id, image, image_id, command, state, status, ports, labels, networks, mounts
    );

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Detail "))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

// ── Networks ───────────────────────────────────────────────────────────────

fn draw_networks(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_network_table(f, app, chunks[0]);
    draw_network_detail(f, app, chunks[1]);
}

fn draw_network_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Driver"),
        Cell::from("Scope"),
        Cell::from("Containers"),
        Cell::from("ID"),
    ])
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    .height(1);

    let rows: Vec<Row> = app
        .networks
        .iter()
        .map(|n| {
            let container_count = n
                .containers
                .as_ref()
                .map(|c| c.len().to_string())
                .unwrap_or_else(|| "0".into());

            Row::new(vec![
                Cell::from(n.name.clone().unwrap_or_default()),
                Cell::from(n.driver.clone().unwrap_or_default()),
                Cell::from(n.scope.clone().unwrap_or_default()),
                Cell::from(container_count),
                Cell::from(
                    n.id.as_deref()
                        .map(|id| if id.len() > 12 { &id[..12] } else { id })
                        .unwrap_or("")
                        .to_string(),
                ),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Networks "),
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    f.render_stateful_widget(table, area, &mut app.network_table_state);
}

fn draw_network_detail(f: &mut Frame, app: &App, area: Rect) {
    let selected = app
        .network_table_state
        .selected()
        .and_then(|i| app.networks.get(i));

    let text = if let Some(n) = selected {
        let containers = n
            .containers
            .as_ref()
            .map(|c| {
                if c.is_empty() {
                    "  (none)".to_string()
                } else {
                    c.iter()
                        .map(|(id, info)| {
                            let name = info.name.as_deref().unwrap_or("unnamed");
                            let short = if id.len() > 12 { &id[..12] } else { id };
                            format!("  {} ({})", name, short)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            })
            .unwrap_or_else(|| "  (none)".into());

        format!(
            "Connected Containers:\n{}",
            containers
        )
    } else {
        "No network selected".into()
    };

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Network Details "),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

// ── Logs ───────────────────────────────────────────────────────────────────

fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let inner_height = area.height.saturating_sub(2) as usize; // borders
    let total = app.log_lines.len();
    let start = app.log_scroll.min(total);
    let end = (start + inner_height).min(total);

    let lines: Vec<Line> = app.log_lines[start..end]
        .iter()
        .map(|l| Line::from(l.as_str()))
        .collect();

    let title = if app.log_streaming {
        " Logs (streaming) - Esc to exit "
    } else {
        " Logs (ended) - Esc to exit "
    };

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(paragraph, area);
}

// ── Status Bar ─────────────────────────────────────────────────────────────

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let keybinds = match app.mode {
        Mode::Logs => "PgUp/PgDn: Scroll | g/G: Top/Bottom | Esc: Back",
        Mode::Detail => "Esc: Back | l: Logs | s: Start/Stop | x: Kill | r: Remove",
        Mode::Normal => match app.tab {
            Tab::Containers => {
                "q: Quit | Tab: Switch | j/k: Navigate | Enter: Detail | l: Logs | s: Start/Stop | x: Kill | r: Remove"
            }
            Tab::Networks => "q: Quit | Tab: Switch | j/k: Navigate",
        },
    };

    let status_text = if let Some(msg) = &app.status_message {
        format!("{} | {}", msg, keybinds)
    } else {
        keybinds.to_string()
    };

    let paragraph = Paragraph::new(Line::from(vec![Span::styled(
        status_text,
        Style::default().fg(Color::White),
    )]))
    .block(Block::default().borders(Borders::ALL));

    f.render_widget(paragraph, area);
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn state_color(state: &str) -> Color {
    match state {
        "running" => Color::Green,
        "exited" | "dead" => Color::Red,
        "paused" => Color::Yellow,
        "restarting" => Color::Cyan,
        "created" => Color::Blue,
        _ => Color::DarkGray,
    }
}
