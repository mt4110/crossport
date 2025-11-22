use crate::core::process::{ProcessInfo, ProcessKind};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use sysinfo::{Pid, System, Users};

pub struct SystemSnapshot {
    pub processes_by_port: HashMap<u16, Vec<ProcessInfo>>,
}

impl SystemSnapshot {
    pub fn capture() -> Result<Self> {
        let mut sys = System::new_all();
        sys.refresh_all();
        let users = Users::new_with_refreshed_list();

        // 1. Get all listening ports and PIDs
        let ports_pids = get_all_listening_ports()?;

        // 2. Get Docker info
        let docker_map = get_docker_containers().unwrap_or_default();

        let mut processes_by_port = HashMap::new();

        // 3. Enrich
        for (pid_val, port) in ports_pids {
            if let Some(info) = enrich_process_info(&sys, &users, pid_val, port, &docker_map) {
                processes_by_port
                    .entry(port)
                    .or_insert_with(Vec::new)
                    .push(info);
            }
        }

        Ok(Self { processes_by_port })
    }

    pub fn get_process_info(&self, port: u16) -> Vec<ProcessInfo> {
        self.processes_by_port
            .get(&port)
            .cloned()
            .unwrap_or_default()
    }
}

fn enrich_process_info(
    sys: &System,
    users: &Users,
    pid_val: u32,
    port: u16,
    docker_map: &HashMap<u16, String>,
) -> Option<ProcessInfo> {
    let pid = Pid::from(pid_val as usize);
    let process = sys.process(pid)?;

    let user = if let Some(uid) = process.user_id() {
        users
            .iter()
            .find(|u| u.id() == uid)
            .map(|u| u.name().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        "unknown".to_string()
    };

    // TODO: Extract numeric UID from sysinfo for proper permission checks
    let uid_val = None;

    let cmd = process.name().to_string();
    let cwd = process.cwd().map(|p| p.to_path_buf()).unwrap_or_default();

    // Determine kind
    let mut kind = determine_kind(&cmd, &cwd, &user);

    // Try to find project root (git)
    let project_root = find_git_root(&cwd);

    // Check Docker map
    let mut container_name = None;
    if let Some(name) = docker_map.get(&port) {
        container_name = Some(name.clone());
        kind = ProcessKind::Docker;
    }

    Some(ProcessInfo {
        pid: pid_val,
        user,
        uid: uid_val,
        cmd,
        cwd,
        project_root,
        container_name,
        kind,
        port,
    })
}

fn get_docker_containers() -> Result<HashMap<u16, String>> {
    // docker ps --format "{{.Names}}\t{{.Ports}}"
    // Output example:
    // my-container	0.0.0.0:8080->80/tcp, :::8080->80/tcp

    let output = Command::new("docker")
        .arg("ps")
        .arg("--format")
        .arg("{{.Names}}\t{{.Ports}}")
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut map = HashMap::new();

            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    let name = parts[0];
                    let ports_str = parts[1];

                    // Parse ports
                    // 0.0.0.0:8080->80/tcp
                    // 0.0.0.0:5432->5432/tcp, :::5432->5432/tcp

                    for port_def in ports_str.split(',') {
                        if let Some(host_part) = port_def.split("->").next() {
                            // host_part: 0.0.0.0:8080 or :::8080
                            if let Some(port_str) = host_part.split(':').next_back() {
                                if let Ok(port) = port_str.parse::<u16>() {
                                    map.insert(port, name.to_string());
                                }
                            }
                        }
                    }
                }
            }
            Ok(map)
        }
        Err(_) => Ok(HashMap::new()), // Docker not installed or not running
    }
}

fn get_all_listening_ports() -> Result<Vec<(u32, u16)>> {
    if cfg!(target_os = "windows") {
        scan_ports_windows()
    } else {
        scan_ports_unix()
    }
}

fn scan_ports_unix() -> Result<Vec<(u32, u16)>> {
    let output = Command::new("lsof")
        .arg("-iTCP")
        .arg("-sTCP:LISTEN")
        .arg("-P")
        .arg("-n")
        .arg("-F")
        .arg("pn") // Request PID ('p') and Name ('n') for network files
        .output()
        .context("Failed to execute lsof for scanning")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    let mut current_pid = None;

    for line in stdout.lines() {
        if let Some(stripped) = line.strip_prefix('p') {
            current_pid = stripped.parse::<u32>().ok();
        } else if let Some(stripped) = line.strip_prefix('n') {
            if let Some(pid) = current_pid {
                // Example: n*:12345
                let port_part = stripped.split(':').next_back().unwrap_or("");
                if let Ok(port) = port_part.parse::<u16>() {
                    results.push((pid, port));
                }
            }
        }
    }
    Ok(results)
}

fn scan_ports_windows() -> Result<Vec<(u32, u16)>> {
    let output = Command::new("netstat")
        .arg("-ano")
        .output()
        .context("Failed to execute netstat")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines() {
        if line.contains("LISTENING") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Expected format: Proto LocalAddress ForeignAddress State PID
            // Example: TCP    0.0.0.0:8088           0.0.0.0:0              LISTENING       31715
            if parts.len() >= 5 {
                let local_addr = parts[1];
                let pid_str = parts[parts.len() - 1];

                if let Ok(port) = local_addr
                    .split(':')
                    .next_back()
                    .unwrap_or("")
                    .parse::<u16>()
                {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        results.push((pid, port));
                    }
                }
            }
        }
    }
    Ok(results)
}

fn determine_kind(cmd: &str, cwd: &std::path::Path, _user: &str) -> ProcessKind {
    let cwd_str = cwd.to_string_lossy();

    if cmd.contains("docker") || cmd.contains("containerd") {
        return ProcessKind::Docker;
    }

    if cwd_str.contains("/_workspace/") || cwd_str.contains("/_projects/") {
        return ProcessKind::Dev;
    }

    if cwd_str.starts_with("/usr/sbin") || cwd_str.starts_with("/System") {
        return ProcessKind::System;
    }

    if cwd_str.contains("/opt/homebrew") || cwd_str.contains("/usr/local/Cellar") {
        return ProcessKind::Brew;
    }

    ProcessKind::Other
}

fn find_git_root(start_path: &std::path::Path) -> Option<PathBuf> {
    if start_path.as_os_str().is_empty() {
        return None;
    }
    let mut current = start_path;
    loop {
        if current.join(".git").exists() {
            return Some(current.to_path_buf());
        }
        match current.parent() {
            Some(p) => current = p,
            None => break,
        }
    }
    None
}
