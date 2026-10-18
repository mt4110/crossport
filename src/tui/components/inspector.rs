use crate::core::{ProcessInfo, SystemSnapshot};
use ratatui::{
    Frame,
    layout::Rect,
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

pub struct Inspector;

impl Inspector {
    pub fn render(
        f: &mut Frame,
        process: &ProcessInfo,
        snapshot: &SystemSnapshot,
        scroll: &mut u16,
        max_scroll: &mut u16,
        confirm: bool,
        error: Option<&str>,
    ) {
        let area = crate::tui::utils::centered_rect(95, 90, f.area());
        let title = if confirm {
            "Stop entire process?"
        } else {
            "Process Details"
        };
        let block = Block::default().title(title).borders(Borders::ALL);
        let inner = block.inner(area);
        let body = Rect {
            height: inner.height.saturating_sub(2),
            ..inner
        };
        let footer = Rect {
            y: inner.y + body.height,
            height: inner.height.min(2),
            ..inner
        };
        let mut endpoints: Vec<_> = snapshot
            .processes_by_port
            .values()
            .flatten()
            .filter(|p| p.pid == process.pid && p.started_at == process.started_at)
            .map(|p| format!("{}:{}", p.local_addr.as_deref().unwrap_or("?"), p.port))
            .collect();
        endpoints.sort();
        endpoints.dedup();
        let path = |p: Option<&std::path::Path>| {
            p.filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Unknown".into())
        };
        let app = process
            .executable
            .as_ref()
            .and_then(|p| {
                p.ancestors()
                    .find(|p| p.extension().is_some_and(|e| e == "app"))
            })
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Unknown / not an app bundle".into());
        let uptime = if process.started_at == 0 {
            "Unknown".into()
        } else {
            format!(
                "{}h {}m {}s (at selection)",
                process.uptime_secs / 3600,
                process.uptime_secs / 60 % 60,
                process.uptime_secs % 60
            )
        };
        let state = if endpoints.is_empty() {
            "No TCP listener currently observed; usage unknown"
        } else {
            "TCP listening at snapshot; usage unknown"
        };
        let mut text = format!(
            "Process: {} (PID {})\nUser: {} / UID: {}\nState: {}\nLast used: Unknown (not monitored)\nUptime: {}\nExecutable: {}\nApp bundle: {}\nParent: {} (PID {})\nProject: {}\nContainer / K8s target: {}\nCWD: {}\nListening endpoints (entire PID): {}\nAuto-restart: Unknown\nKind: {} (heuristic; not a safety assessment)\n\nArguments:\n{}",
            process.cmd,
            process.pid,
            process.user,
            process
                .uid
                .map(|uid| uid.to_string())
                .unwrap_or_else(|| "Unknown / non-Unix".into()),
            state,
            uptime,
            path(process.executable.as_deref()),
            app,
            process.parent_name.as_deref().unwrap_or("Unknown"),
            process
                .parent_pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "Unknown".into()),
            path(process.project_root.as_deref()),
            process
                .container_name
                .as_deref()
                .unwrap_or("None / unknown"),
            path(Some(&process.cwd)),
            if endpoints.is_empty() {
                "No longer observed".into()
            } else {
                endpoints.join(", ")
            },
            process.kind.as_str(),
            if process.args.is_empty() {
                "Unknown".into()
            } else {
                process.args.join("\n")
            }
        );
        if confirm {
            text = format!(
                "This stops the entire process, affecting ALL its ports and work.\nIt does not delete files. Unsaved work may be lost.\nGraceful termination may escalate to forced termination.\n\n{text}"
            );
        }
        if let Some(error) = error {
            text = format!("FAILED: {error}\n\n{text}");
        }
        let text = wrap_text(&text, body.width);
        let line_count = text.lines().count();
        let paragraph = Paragraph::new(text);
        *max_scroll = line_count
            .saturating_sub(body.height as usize)
            .min(u16::MAX as usize) as u16;
        *scroll = (*scroll).min(*max_scroll);
        f.render_widget(Clear, area);
        f.render_widget(block, area);
        f.render_widget(paragraph.scroll((*scroll, 0)), body);
        f.render_widget(
            Paragraph::new(if confirm {
                "j/k or arrows: Scroll | y: Stop | n/Esc: Cancel"
            } else {
                "j/k or arrows: Scroll | PgUp/PgDn | Esc/q: Close"
            })
            .wrap(Wrap { trim: false }),
            footer,
        );
    }
}

// Hard-wrap paths and arguments as well as prose, keeping scroll bounds exact.
pub(crate) fn wrap_text(text: &str, width: u16) -> String {
    let width = usize::from(width.max(1));
    let mut result = String::new();
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            result.push('\n');
        }
        let mut used = 0;
        for c in line.chars() {
            let size = Span::raw(c.to_string()).width();
            if used > 0 && used + size > width {
                result.push('\n');
                used = 0;
            }
            result.push(c);
            used += size;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::wrap_text;
    #[test]
    fn wraps_paths_and_japanese_without_losing_characters() {
        let source = "/long/path/to/server\n日本語の情報";
        let wrapped = wrap_text(source, 6);
        assert_eq!(wrapped.replace('\n', ""), source.replace('\n', ""));
        assert!(
            wrapped
                .lines()
                .all(|line| ratatui::text::Span::raw(line).width() <= 6)
        );
    }
}
