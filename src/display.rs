use crate::core::ProcessInfo;
use colored::*;

pub fn print_process_info(info: &ProcessInfo) {
    println!(
        "{} {}",
        "port".green().bold(),
        info.port.to_string().green().bold()
    );
    println!("  {:<9}: {}", "pid", info.pid);
    println!("  {:<9}: {}", "user", info.user);
    println!("  {:<9}: {}", "cmd", info.cmd);
    println!("  {:<9}: {}", "cwd", info.cwd.display());

    if let Some(root) = &info.project_root {
        println!("  {:<9}: {} (git)", "project", root.display());
    }

    let kind_str = format!("[{}]", info.kind.as_str());
    println!("  {:<9}: {}", "kind", kind_str.cyan());

    // Notes?
    println!();
}

pub fn print_scan_result(infos: &[ProcessInfo]) {
    if infos.is_empty() {
        println!("No listening ports found in range.");
        return;
    }

    println!(
        "{:<6} {:<8} {:<8} {:<8} {:<8} {}",
        "PORT", "PID", "USER", "CMD", "KIND", "PROJ"
    );
    for info in infos {
        let proj = if let Some(container) = &info.container_name {
            container.clone()
        } else {
            info.project_root
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        println!(
            "{:<6} {:<8} {:<8} {:<8} {:<8} {}",
            info.port,
            info.pid,
            truncate(&info.user, 8),
            truncate(&info.cmd, 8),
            info.kind.as_str(),
            proj
        );
    }
}

fn truncate(s: &str, max_width: usize) -> String {
    if s.len() > max_width {
        format!("{}...", &s[0..max_width - 3])
    } else {
        s.to_string()
    }
}
