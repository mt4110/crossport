use crate::core::ProcessInfo;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

pub enum SortColumn {
    Port,
    Pid,
    User,
    Cmd,
    Kind,
    Bind,
    Proj,
}

pub enum SortOrder {
    Asc,
    Desc,
}

pub struct ProcessTable {
    pub state: TableState,
    pub sort_col: SortColumn,
    pub sort_order: SortOrder,
}

impl Default for ProcessTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessTable {
    pub fn new() -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        Self {
            state,
            sort_col: SortColumn::Port,
            sort_order: SortOrder::Asc,
        }
    }

    pub fn sort(&self, processes: &mut [ProcessInfo]) {
        processes.sort_by(|a, b| {
            let res = match self.sort_col {
                SortColumn::Port => a.port.cmp(&b.port),
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::User => a.user.cmp(&b.user),
                SortColumn::Cmd => a.cmd.cmp(&b.cmd),
                SortColumn::Kind => a.kind.as_str().cmp(b.kind.as_str()),
                SortColumn::Bind => {
                    let a_bind = a.local_addr.as_deref().unwrap_or("");
                    let b_bind = b.local_addr.as_deref().unwrap_or("");
                    a_bind.cmp(b_bind)
                }
                SortColumn::Proj => {
                    let a_proj = a.container_name.as_deref().unwrap_or("");
                    let b_proj = b.container_name.as_deref().unwrap_or("");
                    // Fallback to project root if container name empty?
                    // Simplified for sort:
                    a_proj.cmp(b_proj)
                }
            };
            match self.sort_order {
                SortOrder::Asc => res,
                SortOrder::Desc => res.reverse(),
            }
        });
    }

    pub fn next_sort_col(&mut self) {
        self.sort_col = match self.sort_col {
            SortColumn::Port => SortColumn::Pid,
            SortColumn::Pid => SortColumn::User,
            SortColumn::User => SortColumn::Cmd,
            SortColumn::Cmd => SortColumn::Kind,
            SortColumn::Kind => SortColumn::Bind,
            SortColumn::Bind => SortColumn::Proj,
            SortColumn::Proj => SortColumn::Port,
        };
    }

    pub fn toggle_sort_order(&mut self) {
        self.sort_order = match self.sort_order {
            SortOrder::Asc => SortOrder::Desc,
            SortOrder::Desc => SortOrder::Asc,
        };
    }

    pub fn next(&mut self, items_len: usize) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= items_len.saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self, items_len: usize) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    items_len.saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.state.select(index);
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, processes: &[ProcessInfo]) {
        let selected_style = Style::default().add_modifier(Modifier::REVERSED);
        let normal_style = Style::default().bg(Color::Blue);
        let header_cells = [
            (SortColumn::Port, "PORT"),
            (SortColumn::Pid, "PID"),
            (SortColumn::User, "USER"),
            (SortColumn::Cmd, "CMD"),
            (SortColumn::Kind, "KIND"),
            (SortColumn::Bind, "BIND"),
            (SortColumn::Proj, "PROJ"),
        ]
        .iter()
        .map(|(col, h)| {
            let mut style = Style::default().fg(Color::Red);
            let mut text = h.to_string();
            // Discriminant check if we could derive PartialEq but manual for now
            if std::mem::discriminant(col) == std::mem::discriminant(&self.sort_col) {
                style = style.add_modifier(Modifier::BOLD);
                match self.sort_order {
                    SortOrder::Asc => text.push('▲'),
                    SortOrder::Desc => text.push('▼'),
                }
            }
            ratatui::widgets::Cell::from(text).style(style)
        });
        let header = Row::new(header_cells)
            .style(normal_style)
            .height(1)
            .bottom_margin(0);

        let rows = processes.iter().map(|item| {
            let proj = if let Some(container) = &item.container_name {
                container.clone()
            } else {
                item.project_root
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            };

            let bind = item.local_addr.clone().unwrap_or_default();

            let cells = vec![
                item.port.to_string(),
                item.pid.to_string(),
                item.user.clone(),
                item.cmd.clone(),
                item.kind.as_str().to_string(),
                bind,
                proj,
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
                Constraint::Length(15), // BIND
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

        f.render_stateful_widget(t, area, &mut self.state);
    }
}
