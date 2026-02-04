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
        self.process_table.next_sort_col();
        self.process_table.sort(&mut self.processes);
    }

    pub fn toggle_sort_order(&mut self) {
        self.process_table.toggle_sort_order();
        self.process_table.sort(&mut self.processes);
    }

    pub fn kill_selected(&mut self) {
        if let Some(index) = self.process_table.selected() {
            if let Some(proc) = self.processes.get(index) {
                self.input_mode = InputMode::ConfirmKill(proc.pid);
            }
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
        if let Some(index) = self.process_table.selected() {
            if let Some(proc) = self.processes.get(index) {
                self.input_mode = InputMode::Inspecting(proc.clone());
            }
        }
    }

    pub fn exit_inspect_mode(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn restart_selected(&mut self) {
        if let Some(index) = self.process_table.selected() {
            if let Some(proc) = self.processes.get(index) {
                if let Some(container) = &proc.container_name {
                    if proc.kind == crate::core::ProcessKind::Docker
                        || proc.kind == crate::core::ProcessKind::Kubernetes
                    {
                        self.input_mode = InputMode::ConfirmRestart(container.clone());
                    }
                }
            }
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

        // Preserve selection
        let selected_pid = self
            .process_table
            .selected()
            .and_then(|i| self.processes.get(i).map(|p| p.pid));

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
                        || info.kind.as_str().contains(&query)
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
        // Apply sort
        self.process_table.sort(&mut processes);

        self.processes = processes;
        self.snapshot = snapshot;
        self.last_refresh = Instant::now();

        // Restore selection
        if let Some(pid) = selected_pid {
            if let Some(pos) = self.processes.iter().position(|p| p.pid == pid) {
                self.process_table.select(Some(pos));
            } else {
                // If selected process is gone, keep index or clamp
                let current = self.process_table.selected().unwrap_or(0);
                self.process_table
                    .select(Some(current.min(self.processes.len().saturating_sub(1))));
            }
        } else {
            self.process_table.select(Some(0));
        }

        Ok(())
    }

    pub fn on_tick(&mut self) {
        let _ = self.refresh(false);
    }
}
