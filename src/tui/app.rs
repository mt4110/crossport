use crate::core::{ProcessInfo, SystemSnapshot};
use crate::tui::components::process_table::ProcessTable;
use anyhow::Result;

pub enum InputMode {
    Normal,
    EditingFilter,
    Inspecting(ProcessInfo),
    ConfirmKill(ProcessInfo),
    ConfirmRestart(ProcessInfo),
    History,
    Message(String),
    CompletedWithWarning(String),
}

use std::time::{Duration, Instant};

pub struct App {
    pub process_table: ProcessTable,
    pub processes: Vec<ProcessInfo>,
    pub snapshot: SystemSnapshot,
    pub input_mode: InputMode,
    pub filter_query: String,
    pub last_refresh: Instant,
    pub detail_scroll: u16,
    pub detail_max_scroll: u16,
    pub action_error: Option<String>,
    pub settings: crate::config::Config,
    pub events: Vec<crate::history::Event>,
}

impl App {
    pub fn new(settings: crate::config::Config) -> Result<Self> {
        let snapshot = SystemSnapshot::capture_with(settings.backend.strategy)?;
        let events = crate::history::read(&settings.history);
        let history_error = events
            .as_ref()
            .err()
            .map(|error| format!("History load failed: {error}"));
        let events = events.unwrap_or_default();
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
            detail_scroll: 0,
            detail_max_scroll: 0,
            action_error: history_error,
            settings,
            events,
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
            self.detail_scroll = 0;
            self.action_error = None;
            self.input_mode = InputMode::ConfirmKill(proc.clone());
        }
    }

    pub fn confirm_kill(&mut self) -> Result<()> {
        if let InputMode::ConfirmKill(target) = &self.input_mode {
            let target = target.clone();
            let result = crate::permissions::ensure_owned(&target, false)
                .and_then(|_| crate::ops::terminate_process(target.pid, None, false, false, false));
            self.record_action(&target, crate::history::Action::Stop, &result);
            result?;
            self.finish_successful_action("Stop", |app| app.refresh(true));
        }
        Ok(())
    }

    fn record_action(
        &mut self,
        target: &ProcessInfo,
        action: crate::history::Action,
        result: &Result<()>,
    ) {
        let outcome = if result.is_ok() {
            crate::history::Outcome::Success
        } else {
            crate::history::Outcome::Failed
        };
        let event = crate::history::Event::new(target.pid, target.port, action, outcome);
        if let Err(error) = crate::history::record(&self.settings.history, &event) {
            self.action_error = Some(format!(
                "History save failed (operation is not retried): {error}"
            ));
        }
        self.events.push(event);
        let keep = self.settings.history.max_entries;
        if self.events.len() > keep {
            self.events.drain(..self.events.len() - keep);
        }
    }

    pub fn show_history(&mut self) {
        self.detail_scroll = 0;
        self.input_mode = InputMode::History;
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
            self.detail_scroll = 0;
            self.input_mode = InputMode::Inspecting(proc.clone());
        }
    }

    pub fn exit_inspect_mode(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn restart_selected(&mut self) {
        if let Some(index) = self.process_table.selected()
            && let Some(proc) = self.processes.get(index)
            && proc.container_name.is_some()
            && (proc.kind == crate::core::ProcessKind::Docker
                || proc.kind == crate::core::ProcessKind::Kubernetes)
        {
            self.action_error = None;
            self.input_mode = InputMode::ConfirmRestart(proc.clone());
        }
    }

    pub fn confirm_restart(&mut self) -> Result<()> {
        self.restart_with(crate::ops::restart_container)
    }

    fn restart_with(&mut self, restart: impl FnOnce(&str) -> Result<()>) -> Result<()> {
        self.restart_with_refresh(restart, |app| app.refresh(true))
    }

    fn restart_with_refresh(
        &mut self,
        restart: impl FnOnce(&str) -> Result<()>,
        refresh: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        if let InputMode::ConfirmRestart(target) = &self.input_mode {
            let target = target.clone();
            let result = target
                .container_name
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("No container target"))
                .and_then(restart);
            self.record_action(&target, crate::history::Action::Restart, &result);
            if let Err(error) = result {
                self.input_mode = InputMode::Message(error.to_string());
                self.detail_scroll = 0;
                return Err(error);
            }
            self.finish_successful_action("Restart", refresh);
        }
        Ok(())
    }

    fn finish_successful_action(
        &mut self,
        action: &str,
        refresh: impl FnOnce(&mut Self) -> Result<()>,
    ) {
        self.input_mode = InputMode::Normal;
        if let Err(error) = refresh(self) {
            self.detail_scroll = 0;
            self.input_mode = InputMode::CompletedWithWarning(format!(
                "{action} request completed, but refreshing ports failed.\nDisplayed port data may be stale. Do not repeat the action just to refresh.\n\n{error}"
            ));
        }
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

        let snapshot = SystemSnapshot::capture_with(self.settings.backend.strategy)?;
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
            executable: None,
            parent_pid: None,
            parent_name: None,
            started_at: 0,
            uptime_secs: 0,
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
            detail_scroll: 0,
            detail_max_scroll: 0,
            action_error: None,
            settings: crate::config::Config::default(),
            events: Vec::new(),
        }
    }

    #[test]
    fn restart_failure_is_visible_recorded_and_cannot_be_confirmed_again() {
        let mut target = process(10, 3000);
        target.container_name = Some("fixture".into());
        let mut app = app(vec![target.clone()]);
        app.input_mode = InputMode::ConfirmRestart(target);
        assert!(
            app.restart_with(|_| Err(anyhow::anyhow!("fixture restart failure")))
                .is_err()
        );
        assert!(
            matches!(&app.input_mode, InputMode::Message(text) if text.contains("fixture restart failure"))
        );
        assert_eq!(app.events.len(), 1);
        assert_eq!(app.events[0].outcome, crate::history::Outcome::Failed);
        app.restart_with(|_| panic!("must not retry an acknowledged failure"))
            .unwrap();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::tui::ui::ui(frame, &mut app))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("fixture restart failure"));
    }

    #[test]
    fn successful_restart_with_failed_refresh_is_not_reported_as_action_failure() {
        let mut target = process(10, 3000);
        target.container_name = Some("fixture".into());
        let mut app = app(vec![target.clone()]);
        app.input_mode = InputMode::ConfirmRestart(target);
        app.restart_with_refresh(
            |_| Ok(()),
            |_| Err(anyhow::anyhow!("fixture refresh failed")),
        )
        .unwrap();
        assert_eq!(app.events.len(), 1);
        assert_eq!(app.events[0].outcome, crate::history::Outcome::Success);
        assert!(
            matches!(&app.input_mode,InputMode::CompletedWithWarning(text) if text.contains("Restart request completed") && text.contains("fixture refresh failed"))
        );
        app.restart_with_refresh(
            |_| panic!("must not restart twice"),
            |_| panic!("must not refresh through confirmation twice"),
        )
        .unwrap();
        assert_eq!(app.events.len(), 1);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::tui::ui::ui(frame, &mut app))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Action completed / refresh failed"));
        assert!(text.contains("fixture refresh failed"));
        assert!(!text.contains("Action failed"));
    }

    #[test]
    fn successful_stop_refresh_warning_preserves_success_and_old_snapshot() {
        let mut app = app(vec![process(10, 3000)]);
        let target = app.processes[0].clone();
        app.record_action(&target, crate::history::Action::Stop, &Ok(()));
        app.finish_successful_action("Stop", |_| Err(anyhow::anyhow!("fixture refresh failed")));
        assert!(
            matches!(&app.input_mode,InputMode::CompletedWithWarning(text) if text.contains("Stop request completed"))
        );
        assert_eq!(app.events[0].outcome, crate::history::Outcome::Success);
        assert_eq!(app.processes[0].pid, 10);
        app.exit_inspect_mode();
        assert!(matches!(app.input_mode, InputMode::Normal));
    }

    #[test]
    fn color_disabled_resets_every_rendered_cell_including_modal() {
        let mut app = app(vec![process(10, 3000)]);
        app.settings.output.color = Some(true);
        app.settings.ui.color = Some(false);
        app.input_mode = InputMode::Message("failure details".into());
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::tui::ui::ui(frame, &mut app))
            .unwrap();
        for cell in &terminal.backend().buffer().content {
            assert_eq!(cell.fg, ratatui::style::Color::Reset);
            assert_eq!(cell.bg, ratatui::style::Color::Reset);
        }
    }

    #[test]
    fn confirmation_keeps_original_target_when_selection_changes() {
        let mut app = app(vec![process(10, 3000), process(20, 3001)]);
        app.process_table.select(Some(0));
        app.kill_selected();
        app.replace_processes(vec![process(20, 3001)]);
        assert!(matches!(&app.input_mode, InputMode::ConfirmKill(p) if p.pid == 10));
        app.cancel_kill();
        assert!(matches!(app.input_mode, InputMode::Normal));
    }

    #[test]
    fn missing_process_identity_blocks_termination() {
        let mut app = app(vec![process(std::process::id(), 3000)]);
        app.process_table.select(Some(0));
        app.kill_selected();
        assert!(app.confirm_kill().is_err());
        assert!(matches!(app.input_mode, InputMode::ConfirmKill(_)));
    }

    #[test]
    fn changed_start_time_blocks_termination_of_existing_pid() {
        let system = sysinfo::System::new_all();
        let current = system
            .process(sysinfo::Pid::from_u32(std::process::id()))
            .unwrap();
        let mut target = process(std::process::id(), 3000);
        target.started_at = current.start_time().saturating_sub(1);
        target.executable = current.exe().map(PathBuf::from);
        let mut app = app(vec![target]);
        app.process_table.select(Some(0));
        app.kill_selected();
        assert!(app.confirm_kill().is_err());
        assert!(matches!(app.input_mode, InputMode::ConfirmKill(_)));
    }

    #[test]
    fn confirmation_shows_ports_hidden_by_filter_and_deduplicates_endpoints() {
        let mut app = app(vec![process(10, 3000)]);
        app.snapshot
            .processes_by_port
            .insert(3000, vec![process(10, 3000)]);
        app.snapshot
            .processes_by_port
            .insert(3001, vec![process(10, 3001), process(10, 3001)]);
        app.process_table.select(Some(0));
        app.kill_selected();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 50)).unwrap();
        terminal
            .draw(|frame| crate::tui::ui::ui(frame, &mut app))
            .unwrap();
        let contents = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(contents.contains("127.0.0.1:3000, 127.0.0.1:3001"));
        assert!(contents.contains("entire process"));
        assert!(contents.contains("Last used: Unknown"));
    }

    #[test]
    fn details_scroll_to_last_argument_at_standard_terminal_size() {
        let mut proc = process(10, 3000);
        proc.args = vec!["/very/long/path/".repeat(20), "TAILEND".into()];
        let mut app = app(vec![proc]);
        app.process_table.select(Some(0));
        app.inspect_selected();
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| crate::tui::ui::ui(frame, &mut app))
            .unwrap();
        assert!(app.detail_max_scroll > 0);
        app.detail_scroll = u16::MAX;
        terminal
            .draw(|frame| crate::tui::ui::ui(frame, &mut app))
            .unwrap();
        assert_eq!(app.detail_scroll, app.detail_max_scroll);
        let contents = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(contents.contains("TAILEND"));
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
