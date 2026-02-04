use crate::core::ProcessInfo;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub struct Inspector;

impl Inspector {
    pub fn render(f: &mut Frame, _area: Rect, process: &ProcessInfo) {
        let block = Block::default()
            .title(format!(
                "Process Details: {} (PID: {})",
                process.cmd, process.pid
            ))
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black));

        // Calculate centered area
        let area = centered_rect(60, 60, f.size());

        f.render_widget(Clear, area); // Clear background

        // Layout inside popup
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(1), // PID/User
                    Constraint::Length(1), // Port/Bind
                    Constraint::Length(1), // Kind/Project
                    Constraint::Length(1), // CWD
                    Constraint::Length(2), // Separator/Header
                    Constraint::Min(5),    // Args/Env
                ]
                .as_ref(),
            )
            .split(block.inner(area));

        f.render_widget(block, area);

        // Row 1
        let row1 = Line::from(vec![
            Span::styled("PID: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{:<10}", process.pid)),
            Span::styled("User: ", Style::default().fg(Color::Cyan)),
            Span::raw(process.user.to_string()),
        ]);
        f.render_widget(Paragraph::new(row1), layout[0]);

        // Row 2
        let row2 = Line::from(vec![
            Span::styled("Port: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{:<10}", process.port)),
            Span::styled("Bind: ", Style::default().fg(Color::Cyan)),
            Span::raw(process.local_addr.as_deref().unwrap_or("-")),
        ]);
        f.render_widget(Paragraph::new(row2), layout[1]);

        // Row 3
        let row3 = Line::from(vec![
            Span::styled("Kind: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{:<10}", process.kind.as_str())),
            Span::styled("Project: ", Style::default().fg(Color::Cyan)),
            Span::raw(
                process
                    .project_root
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            ),
        ]);
        f.render_widget(Paragraph::new(row3), layout[2]);

        // Row 4
        let row4 = Line::from(vec![
            Span::styled("CWD: ", Style::default().fg(Color::Cyan)),
            Span::raw(process.cwd.display().to_string()),
        ]);
        f.render_widget(Paragraph::new(row4), layout[3]);

        // Row 5 (Args Header)
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                "\nArguments:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )])),
            layout[4],
        );

        // Row 6 (Args List)
        let args_text = process.args.join(" ");
        let args_p = Paragraph::new(args_text).wrap(Wrap { trim: true });
        f.render_widget(args_p, layout[5]);
    }
}

// Helper function (maybe move to utils or generic component helper)
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
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
        .direction(Direction::Horizontal)
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
