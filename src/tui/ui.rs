use crate::tui::app::{App, InputMode};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub fn ui(f: &mut Frame, app: &mut App) {
    let rects = Layout::default()
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .margin(1)
        .split(f.area());

    // Clear screen/background?
    // Actually crossterm handles that with AlternateScreen usually.

    // Table
    app.process_table.render(f, rects[0], &app.processes);

    // Filter / Help Footer
    let filter_text = if let InputMode::EditingFilter = app.input_mode {
        format!("Filter: {}_", app.filter_query)
    } else if let Some(error) = &app.action_error {
        error.clone()
    } else if !app.filter_query.is_empty() {
        format!("Filter: {} (Esc to clear)", app.filter_query)
    } else {
        "Enter: Details | /: Filter | s/S: Sort | x: Stop | r: Restart | h: History | q: Quit"
            .to_string()
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

    if let InputMode::ConfirmKill(proc) = &app.input_mode {
        crate::tui::components::inspector::Inspector::render(
            f,
            proc,
            &app.snapshot,
            &mut app.detail_scroll,
            &mut app.detail_max_scroll,
            true,
            app.action_error.as_deref(),
        );
    }

    if let InputMode::ConfirmRestart(target) = &app.input_mode {
        let block = Block::default()
            .title("Confirm Restart")
            .borders(Borders::ALL);
        let area = crate::tui::utils::centered_rect(90, 50, f.area());
        let text = Paragraph::new(format!(
            "Are you sure you want to restart container '{}'? (y/n)",
            target.container_name.as_deref().unwrap_or("Unknown")
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

    if let InputMode::Message(message) = &app.input_mode {
        render_text(
            f,
            "Action failed (Esc/q: Close)",
            message,
            &mut app.detail_scroll,
            &mut app.detail_max_scroll,
        );
    }
    if let InputMode::CompletedWithWarning(message) = &app.input_mode {
        render_text(
            f,
            "Action completed / refresh failed (Esc/q: Close)",
            message,
            &mut app.detail_scroll,
            &mut app.detail_max_scroll,
        );
    }
    if matches!(app.input_mode, InputMode::History) {
        let text = if app.events.is_empty() {
            "No action history in this session or saved history.".into()
        } else {
            app.events
                .iter()
                .rev()
                .map(|event| event.line())
                .collect::<Vec<_>>()
                .join("\n")
        };
        render_text(
            f,
            "Action history (h) / Esc: Close",
            &text,
            &mut app.detail_scroll,
            &mut app.detail_max_scroll,
        );
    }
    if let InputMode::Inspecting(proc) = &app.input_mode {
        crate::tui::components::inspector::Inspector::render(
            f,
            proc,
            &app.snapshot,
            &mut app.detail_scroll,
            &mut app.detail_max_scroll,
            false,
            None,
        );
    }
    if !app
        .settings
        .ui
        .color
        .or(app.settings.output.color)
        .unwrap_or(true)
    {
        for cell in &mut f.buffer_mut().content {
            cell.set_fg(Color::Reset);
            cell.set_bg(Color::Reset);
        }
    }
}

fn render_text(f: &mut Frame, title: &str, text: &str, scroll: &mut u16, max: &mut u16) {
    let area = crate::tui::utils::centered_rect(95, 90, f.area());
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    let text = crate::tui::components::inspector::wrap_text(text, inner.width);
    *max = text
        .lines()
        .count()
        .saturating_sub(inner.height as usize)
        .min(u16::MAX as usize) as u16;
    *scroll = (*scroll).min(*max);
    f.render_widget(Clear, area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(text).scroll((*scroll, 0)), inner);
}
