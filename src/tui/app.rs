use crate::core::{ProcessInfo, SystemSnapshot};
use anyhow::Result;
use ratatui::widgets::TableState;

pub enum InputMode {
    Normal,
    ConfirmKill(u32),
}

use std::time::{Duration, Instant};

pub struct App {
    pub state: TableState,
    pub processes: Vec<ProcessInfo>,
    pub snapshot: SystemSnapshot,
    pub input_mode: InputMode,
    pub last_refresh: Instant,
}

impl App {
    pub fn new() -> Result<Self> {
        let snapshot = SystemSnapshot::capture()?;
        let mut processes = Vec::new();
        for infos in snapshot.processes_by_port.values() {
            processes.extend(infos.clone());
        }
        processes.sort_by_key(|p| p.port);

        let mut state = TableState::default();
        state.select(Some(0));

        Ok(Self {
            state,
            processes,
            snapshot,
            input_mode: InputMode::Normal,
            last_refresh: Instant::now(),
        })
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.processes.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.processes.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn kill_selected(&mut self) {
        if let Some(index) = self.state.selected() {
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

    pub fn refresh(&mut self, force: bool) -> Result<()> {
        if !force && self.last_refresh.elapsed() < Duration::from_secs(2) {
            return Ok(());
        }

        // Preserve selection
        let selected_pid = self
            .state
            .selected()
            .and_then(|i| self.processes.get(i).map(|p| p.pid));

        let snapshot = SystemSnapshot::capture()?;
        let mut processes = Vec::new();
        for infos in snapshot.processes_by_port.values() {
            processes.extend(infos.clone());
        }
        processes.sort_by_key(|p| p.port);
        self.processes = processes;
        self.snapshot = snapshot;
        self.last_refresh = Instant::now();

        // Restore selection
        if let Some(pid) = selected_pid {
            if let Some(pos) = self.processes.iter().position(|p| p.pid == pid) {
                self.state.select(Some(pos));
            } else {
                // If selected process is gone, keep index or clamp
                let current = self.state.selected().unwrap_or(0);
                self.state
                    .select(Some(current.min(self.processes.len().saturating_sub(1))));
            }
        } else {
            self.state.select(Some(0));
        }

        Ok(())
    }

    pub fn on_tick(&mut self) {
        let _ = self.refresh(false);
    }
}
