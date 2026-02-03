use crate::tui::app::{App, InputMode};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn ui(f: &mut Frame, app: &mut App) {
    let rects = Layout::default()
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .margin(1)
        .split(f.size());

    // Clear screen/background?
    // Actually crossterm handles that with AlternateScreen usually.

    // Table
    app.process_table.render(f, rects[0], &app.processes);

    // Filter / Help Footer
    let filter_text = if let InputMode::EditingFilter = app.input_mode {
        format!("Filter: {}_", app.filter_query)
    } else if !app.filter_query.is_empty() {
        format!("Filter: {} (Esc to clear)", app.filter_query)
    } else {
        "Keybindings: </> Filter | <s> Sort | <S> Rev Sort | <x> Kill | <q> Quit".to_string()
    };

    let footer_style = if let InputMode::EditingFilter = app.input_mode {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let footer = Paragraph::new(filter_text)
        .style(footer_style)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(footer, rects[1]);

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

    if let InputMode::ConfirmRestart(container) = &app.input_mode {
        let block = Block::default()
            .title("Confirm Restart")
            .borders(Borders::ALL);
        let area = centered_rect(60, 20, f.size());
        let text = Paragraph::new(format!(
            "Are you sure you want to restart container '{}'? (y/n)",
            container
        ))
        .block(block)
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(Clear, area); // Clear background
        f.render_widget(text, area);
    }

    if let InputMode::Inspecting(proc) = &app.input_mode {
        crate::tui::components::inspector::Inspector::render(f, f.size(), proc);
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
