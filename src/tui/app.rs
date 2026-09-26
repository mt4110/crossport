use crate::core::{ProcessInfo, SystemSnapshot};
use crate::tui::components::process_table::ProcessTable;
use anyhow::Result;

pub enum InputMode {
    Normal,
    EditingFilter,
    Inspecting(ProcessInfo),
    ConfirmKill(u32),
    ConfirmRestart(String),
}

use std::time::{Duration, Instant};

pub struct App {
    pub process_table: ProcessTable,
    pub processes: Vec<ProcessInfo>,
    pub snapshot: SystemSnapshot,
    pub input_mode: InputMode,
    pub filter_query: String,
    pub last_refresh: Instant,
}

impl App {
    pub fn new() -> Result<Self> {
        let snapshot = SystemSnapshot::capture()?;
        let mut processes = Vec::new();
        for infos in snapshot.processes_by_port.values() {
            processes.extend(infos.clone());
        }

        // Initial sort
        let process_table = ProcessTable::new();
        process_table.sort(&mut processes);

        Ok(Self {
            process_table,
            processes,
            snapshot,
            input_mode: InputMode::Normal,
            filter_query: String::new(),
            last_refresh: Instant::now(),
        })
    }

    pub fn next(&mut self) {
        self.process_table.next(self.processes.len());
    }

    pub fn previous(&mut self) {
        self.process_table.previous(self.processes.len());
    }

    pub fn next_sort_col(&mut self) {
        let selection = self.selection_context();
        self.process_table.next_sort_col();
        self.process_table.sort(&mut self.processes);
        self.restore_selection(selection);
    }

    pub fn toggle_sort_order(&mut self) {
        let selection = self.selection_context();
        self.process_table.toggle_sort_order();
        self.process_table.sort(&mut self.processes);
        self.restore_selection(selection);
    }

    pub fn kill_selected(&mut self) {
        if let Some(index) = self.process_table.selected()
            && let Some(proc) = self.processes.get(index)
        {
            self.input_mode = InputMode::ConfirmKill(proc.pid);
        }
    }

    pub fn confirm_kill(&mut self) -> Result<()> {
        if let InputMode::ConfirmKill(pid) = self.input_mode {
            crate::ops::kill_process(pid, None, false, false)?;
            self.refresh(true)?; // Force refresh after kill
        }
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn cancel_kill(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn enter_filter_mode(&mut self) {
        self.input_mode = InputMode::EditingFilter;
    }

    pub fn exit_filter_mode(&mut self) {
        self.input_mode = InputMode::Normal;
        // Optional: clear filter on valid exit?
        // Usually Esc clears, Enter keeps.
    }

    pub fn inspect_selected(&mut self) {
        if let Some(index) = self.process_table.selected()
            && let Some(proc) = self.processes.get(index)
        {
            self.input_mode = InputMode::Inspecting(proc.clone());
        }
    }

    pub fn exit_inspect_mode(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn restart_selected(&mut self) {
        if let Some(index) = self.process_table.selected()
            && let Some(proc) = self.processes.get(index)
            && let Some(container) = &proc.container_name
            && (proc.kind == crate::core::ProcessKind::Docker
                || proc.kind == crate::core::ProcessKind::Kubernetes)
        {
            self.input_mode = InputMode::ConfirmRestart(container.clone());
        }
    }

    pub fn confirm_restart(&mut self) -> Result<()> {
        if let InputMode::ConfirmRestart(container) = &self.input_mode {
            crate::ops::restart_container(container)?;
            self.refresh(true)?;
        }
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn cancel_restart(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn clear_filter(&mut self) {
        self.filter_query.clear();
        let _ = self.refresh(true);
    }

    pub fn append_filter(&mut self, c: char) {
        self.filter_query.push(c);
        let _ = self.refresh(true);
    }

    pub fn pop_filter(&mut self) {
        self.filter_query.pop();
        let _ = self.refresh(true);
    }

    pub fn refresh(&mut self, force: bool) -> Result<()> {
        if !force && self.last_refresh.elapsed() < Duration::from_secs(2) {
            return Ok(());
        }

        let snapshot = SystemSnapshot::capture()?;
        let mut processes = Vec::new();
        let query = self.filter_query.to_lowercase();

        for infos in snapshot.processes_by_port.values() {
            for info in infos {
                if query.is_empty() {
                    processes.push(info.clone());
                } else {
                    // Simple contains check
                    let matches = info.cmd.to_lowercase().contains(&query)
                        || info.user.to_lowercase().contains(&query)
                        || info.port.to_string().contains(&query)
                        || info.kind.as_str().to_lowercase().contains(&query)
                        || info
                            .container_name
                            .as_ref()
                            .map(|s| s.to_lowercase().contains(&query))
                            .unwrap_or(false)
                        || info
                            .project_root
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_lowercase().contains(&query))
                            .unwrap_or(false);

                    if matches {
                        processes.push(info.clone());
                    }
                }
            }
        }
        self.replace_processes(processes);
        self.snapshot = snapshot;
        self.last_refresh = Instant::now();

        Ok(())
    }

    fn replace_processes(&mut self, mut processes: Vec<ProcessInfo>) {
        let selection = self.selection_context();
        self.process_table.sort(&mut processes);
        self.processes = processes;
        self.restore_selection(selection);
    }

    fn selection_context(&self) -> (Option<(u32, u16, Option<String>)>, usize) {
        let selected_index = self.process_table.selected().unwrap_or(0);
        let selected_row = self
            .processes
            .get(selected_index)
            .map(|process| (process.pid, process.port, process.local_addr.clone()));
        (selected_row, selected_index)
    }

    fn restore_selection(
        &mut self,
        (selected_row, previous_index): (Option<(u32, u16, Option<String>)>, usize),
    ) {
        if self.processes.is_empty() {
            self.process_table.select(None);
            return;
        }

        let index = selected_row
            .and_then(|(pid, port, local_addr)| {
                self.processes.iter().position(|process| {
                    process.pid == pid && process.port == port && process.local_addr == local_addr
                })
            })
            .unwrap_or_else(|| previous_index.min(self.processes.len() - 1));

        self.process_table.select(Some(index));
    }

    pub fn on_tick(&mut self) {
        let _ = self.refresh(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ProcessKind;
    use std::{collections::HashMap, path::PathBuf};

    fn process(pid: u32, port: u16) -> ProcessInfo {
        ProcessInfo {
            pid,
            user: "user".to_string(),
            uid: None,
            cmd: "server".to_string(),
            cwd: PathBuf::from("/tmp"),
            project_root: None,
            container_name: None,
            kind: ProcessKind::Dev,
            port,
            local_addr: Some("127.0.0.1".to_string()),
            args: Vec::new(),
        }
    }

    fn app(processes: Vec<ProcessInfo>) -> App {
        App {
            process_table: ProcessTable::new(),
            processes,
            snapshot: SystemSnapshot {
                processes_by_port: HashMap::new(),
            },
            input_mode: InputMode::Normal,
            filter_query: String::new(),
            last_refresh: Instant::now(),
        }
    }

    #[test]
    fn refresh_preserves_selected_port_when_pid_has_multiple_ports() {
        let mut app = app(vec![
            process(10, 3000),
            process(10, 3001),
            process(10, 3002),
        ]);
        app.process_table.select(Some(2));

        app.replace_processes(vec![
            process(10, 2999),
            process(10, 3000),
            process(10, 3001),
            process(10, 3002),
        ]);

        let selected = app.process_table.selected().unwrap();
        assert_eq!(app.processes[selected].port, 3002);
    }

    #[test]
    fn refresh_uses_nearest_valid_row_when_selected_port_disappears() {
        let mut app = app(vec![
            process(10, 3000),
            process(20, 3001),
            process(30, 3002),
        ]);
        app.process_table.select(Some(1));

        app.replace_processes(vec![process(10, 3000), process(30, 3002)]);

        assert_eq!(app.process_table.selected(), Some(1));
        assert_eq!(app.processes[1].port, 3002);
    }

    #[test]
    fn refresh_clears_selection_for_empty_results() {
        let mut app = app(vec![process(10, 3000)]);

        app.replace_processes(Vec::new());
        app.next();
        app.previous();

        assert_eq!(app.process_table.selected(), None);
    }

    #[test]
    fn sorting_preserves_selected_process_row() {
        let mut app = app(vec![process(10, 3000), process(20, 3001)]);
        app.process_table.select(Some(0));

        app.toggle_sort_order();

        let selected = app.process_table.selected().unwrap();
        assert_eq!(app.processes[selected].pid, 10);
        assert_eq!(app.processes[selected].port, 3000);
    }
}
