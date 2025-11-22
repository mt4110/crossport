use crate::tui::app::{App, InputMode};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table},
    Frame,
};

pub fn ui(f: &mut Frame, app: &mut App) {
    let rects = Layout::default()
        .constraints([Constraint::Percentage(100)].as_ref())
        .margin(1)
        .split(f.size());

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let normal_style = Style::default().bg(Color::Blue);
    let header_cells = ["PORT", "PID", "USER", "CMD", "KIND", "PROJ"]
        .iter()
        .map(|h| ratatui::widgets::Cell::from(*h).style(Style::default().fg(Color::Red)));
    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(0);

    let rows = app.processes.iter().map(|item| {
        let proj = if let Some(container) = &item.container_name {
            container.clone()
        } else {
            item.project_root
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        let cells = vec![
            item.port.to_string(),
            item.pid.to_string(),
            item.user.clone(),
            item.cmd.clone(),
            item.kind.as_str().to_string(),
            proj.to_string(),
        ];
        Row::new(cells).height(1).bottom_margin(0)
    });

    let t = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Length(10),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Crossport TUI (q: Quit, x: Kill)"),
    )
    .highlight_style(selected_style)
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, rects[0], &mut app.state);

    if let InputMode::ConfirmKill(pid) = app.input_mode {
        let block = Block::default().title("Confirm Kill").borders(Borders::ALL);
        let area = centered_rect(60, 20, f.size());
        let text = Paragraph::new(format!(
            "Are you sure you want to kill process {}? (y/n)",
            pid
        ))
        .block(block)
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
        f.render_widget(Clear, area); // Clear background
        f.render_widget(text, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
