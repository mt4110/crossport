use crate::config::BackendStrategy;
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
        Self::capture_with(BackendStrategy::Auto)
    }

    pub fn capture_with(strategy: BackendStrategy) -> Result<Self> {
        let mut sys = System::new_all();
        sys.refresh_all();
        let users = Users::new_with_refreshed_list();

        // 1. Get all listening ports and PIDs
        let ports_pids = get_all_listening_ports(strategy)?;

        // 2. Get Docker info
        let docker_map = get_docker_containers().unwrap_or_default();

        let mut processes_by_port = HashMap::new();

        // 3. Enrich
        for (pid_val, port, local_addr) in ports_pids {
            let info = enrich_process_info(&sys, &users, pid_val, port, local_addr, &docker_map)?;
            processes_by_port
                .entry(port)
                .or_insert_with(Vec::new)
                .push(info);
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
    local_addr: String,
    docker_map: &HashMap<u16, String>,
) -> Result<ProcessInfo> {
    let pid = Pid::from(pid_val as usize);
    let process = sys.process(pid).with_context(|| format!(
        "Scan incomplete: TCP listener {local_addr}:{port} (PID {pid_val}) has unavailable process metadata; cannot report ports as free"
    ))?;

    let user = if let Some(uid) = process.user_id() {
        users
            .iter()
            .find(|u| u.id() == uid)
            .map(|u| u.name().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        "unknown".to_string()
    };

    #[cfg(unix)]
    let uid_val = process.user_id().map(|uid| **uid);
    #[cfg(not(unix))]
    let uid_val = None;

    let cmd = process.name().to_string_lossy().into_owned();
    let args: Vec<String> = process
        .cmd()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let cwd = process.cwd().map(|p| p.to_path_buf()).unwrap_or_default();

    // Determine kind
    let mut kind = determine_kind(&cmd, &cwd, &user);

    // Try to find project root (git)
    let project_root = find_git_root(&cwd);

    let mut container_name = None;

    // Check for Kubernetes
    if cmd == "kubectl" || cmd == "k3s" {
        // Try to parse full command line arguments to find target
        if let Some(args) = args.get(0..) {
            // Try to find "port-forward" and the service/pod name
            // args usually: ["kubectl", "port-forward", "svc/my-api", "8080:80"]
            if args.iter().any(|arg| arg == "port-forward") {
                kind = ProcessKind::Kubernetes;
                // Heuristic: Find the arg that doesn't start with "-" and isn't "port-forward" or "kubectl"
                if let Some(target) = args
                    .iter()
                    .skip_while(|&a| a != "port-forward")
                    .skip(1)
                    .find(|&a| !a.starts_with('-') && !a.contains(':'))
                {
                    container_name = Some(target.clone());
                }
            }
        }
    }

    // Check Docker map
    if let Some(name) = docker_map.get(&port) {
        container_name = Some(name.clone());
        if name.starts_with("k8s_") {
            kind = ProcessKind::Kubernetes;
            // Optional: Extract cleaner name?
            // k8s_container_pod_namespace...
            // For now, let's keep the full name or maybe just the pod name?
            // User might want to know it's a pod.
            // Let's rely on the full name for matching project awareness if possible,
            // but for "PROJ" column, maybe we want Pod Name.
        } else {
            kind = ProcessKind::Docker;
        }
    }

    Ok(ProcessInfo {
        pid: pid_val,
        user,
        uid: uid_val,
        cmd,
        cwd,
        project_root,
        container_name,
        kind,
        port,
        local_addr: if local_addr.is_empty() {
            None
        } else {
            Some(local_addr)
        },
        args,
        executable: process.exe().map(PathBuf::from),
        parent_pid: process.parent().map(|pid| pid.as_u32()),
        parent_name: process
            .parent()
            .and_then(|pid| sys.process(pid))
            .map(|p| p.name().to_string_lossy().into_owned()),
        started_at: process.start_time(),
        uptime_secs: process.run_time(),
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
            anyhow::ensure!(output.status.success(), "Docker metadata unavailable");
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
                            if let Some(port_str) = host_part.split(':').next_back()
                                && let Ok(port) = port_str.parse::<u16>()
                            {
                                map.insert(port, name.to_string());
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

fn get_all_listening_ports(strategy: BackendStrategy) -> Result<Vec<(u32, u16, String)>> {
    match strategy {
        BackendStrategy::Auto if cfg!(windows) => scan_ports_windows(),
        BackendStrategy::Auto | BackendStrategy::Lsof if cfg!(unix) => scan_ports_unix(),
        BackendStrategy::Netstat if cfg!(windows) => scan_ports_windows(),
        BackendStrategy::Netstat | BackendStrategy::Ss if cfg!(target_os = "linux") => {
            scan_ports_linux(strategy)
        }
        _ => anyhow::bail!(
            "Invalid config: backend {:?} is unsupported on this OS",
            strategy
        ),
    }
}

fn scan_ports_linux(strategy: BackendStrategy) -> Result<Vec<(u32, u16, String)>> {
    let (tool, args): (&str, &[&str]) = if strategy == BackendStrategy::Ss {
        ("ss", &["-H", "-lntp"])
    } else {
        ("netstat", &["-lntp"])
    };
    let output = Command::new(tool)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .with_context(|| format!("Failed to execute {tool}"))?;
    anyhow::ensure!(
        output.status.success(),
        "Scan error: {tool} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_linux_listeners(&String::from_utf8_lossy(&output.stdout), strategy)
}

fn parse_linux_listeners(text: &str, strategy: BackendStrategy) -> Result<Vec<(u32, u16, String)>> {
    let mut result = Vec::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        let (endpoint, pids) = if strategy == BackendStrategy::Ss {
            if fields.first() != Some(&"LISTEN") {
                continue;
            }
            anyhow::ensure!(
                fields.len() >= 5,
                "Scan incomplete: malformed ss LISTEN row"
            );
            // Read only the users field after the fixed socket columns. Names may
            // contain whitespace, commas and text resembling process fields.
            let mut rest = line;
            for _ in 0..5 {
                rest = rest.trim_start();
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                rest = &rest[end..];
            }
            let pids = if rest.trim().is_empty() {
                Vec::new()
            } else {
                parse_ss_users(rest.trim())?
            };
            (fields[3], pids)
        } else {
            if fields.first().is_none_or(|f| !f.starts_with("tcp")) {
                continue;
            }
            if !fields.contains(&"LISTEN") {
                continue;
            }
            anyhow::ensure!(
                fields.len() >= 7 && fields[5] == "LISTEN",
                "Scan incomplete: malformed netstat LISTEN row"
            );
            let pids = fields[6]
                .split_once('/')
                .and_then(|(pid, _)| pid.parse::<u32>().ok())
                .filter(|pid| *pid > 0)
                .into_iter()
                .collect();
            (fields[3], pids)
        };
        let (addr, port) = endpoint
            .rsplit_once(':')
            .context("Scan incomplete: malformed listener address")?;
        let port: u16 = port
            .parse()
            .context("Scan incomplete: invalid listener port")?;
        anyhow::ensure!(
            !pids.is_empty(),
            "Scan incomplete: TCP listener {endpoint} exists but its PID is unavailable; cannot report ports as free"
        );
        for pid in pids {
            result.push((pid, port, addr.to_string()));
        }
    }
    result.sort();
    result.dedup();
    Ok(result)
}

// ss -p emits users:(("name",pid=N,fd=N),...). Parse that grammar instead
// of searching for pid= in arbitrary text. A malformed users field is an error.
fn parse_ss_users(text: &str) -> Result<Vec<u32>> {
    let mut rest = text
        .strip_prefix("users:(")
        .context("Scan incomplete: invalid ss users field")?;
    let mut pids = Vec::new();
    loop {
        let name = rest
            .strip_prefix("(\"")
            .context("Scan incomplete: invalid ss process entry")?;
        let bytes = name.as_bytes();
        let mut end = 0;
        while end < bytes.len() {
            match bytes[end] {
                b'\\' => {
                    anyhow::ensure!(
                        end + 1 < bytes.len(),
                        "Scan incomplete: truncated ss name escape"
                    );
                    end += 2;
                }
                b'"' => break,
                _ => end += 1,
            }
        }
        anyhow::ensure!(
            end < bytes.len(),
            "Scan incomplete: unterminated ss process name"
        );
        rest = name[end + 1..]
            .strip_prefix(",pid=")
            .context("Scan incomplete: missing ss PID field")?;
        let (pid, tail) = decimal_prefix(rest)?;
        anyhow::ensure!(pid > 0, "Scan incomplete: invalid ss PID");
        rest = tail
            .strip_prefix(",fd=")
            .context("Scan incomplete: missing ss fd field")?;
        let (_, tail) = decimal_prefix(rest)?;
        rest = tail
            .strip_prefix(')')
            .context("Scan incomplete: invalid ss entry delimiter")?;
        pids.push(pid);
        if let Some(tail) = rest.strip_prefix(')') {
            anyhow::ensure!(
                tail.trim().is_empty(),
                "Scan incomplete: unexpected ss users suffix"
            );
            return Ok(pids);
        }
        rest = rest
            .strip_prefix(',')
            .context("Scan incomplete: invalid ss users delimiter")?;
    }
}

fn decimal_prefix(text: &str) -> Result<(u32, &str)> {
    let length = text.bytes().take_while(u8::is_ascii_digit).count();
    anyhow::ensure!(length > 0, "Scan incomplete: missing numeric process field");
    let value = text[..length]
        .parse()
        .context("Scan incomplete: invalid numeric process field")?;
    Ok((value, &text[length..]))
}

fn scan_ports_unix() -> Result<Vec<(u32, u16, String)>> {
    let output = Command::new("lsof")
        .arg("-iTCP")
        .arg("-sTCP:LISTEN")
        .arg("-P")
        .arg("-n")
        .arg("-F")
        .arg("pn") // Request PID ('p') and Name ('n') for network files
        .output()
        .context("Failed to execute lsof for scanning")?;

    anyhow::ensure!(
        output.status.success()
            || (output.status.code() == Some(1)
                && output.stdout.is_empty()
                && output.stderr.is_empty()),
        "Scan error: command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    let mut current_pid = None;

    for line in stdout.lines() {
        if let Some(stripped) = line.strip_prefix('p') {
            current_pid = stripped.parse::<u32>().ok();
        } else if let Some(stripped) = line.strip_prefix('n')
            && let Some(pid) = current_pid
        {
            // Example: n*:12345
            // n127.0.0.1:9277
            let (addr, port_str) = if let Some(idx) = stripped.rfind("]:") {
                (&stripped[..idx + 1], &stripped[idx + 2..])
            } else if let Some(pair) = stripped.rsplit_once(':') {
                pair
            } else {
                continue;
            };

            if let Ok(port) = port_str.parse::<u16>() {
                results.push((pid, port, addr.to_string()));
            }
        }
    }
    Ok(results)
}

fn scan_ports_windows() -> Result<Vec<(u32, u16, String)>> {
    let output = Command::new("netstat")
        .arg("-ano")
        .output()
        .context("Failed to execute netstat")?;

    anyhow::ensure!(
        output.status.success(),
        "Scan error: netstat failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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

                if let Some((addr, port_str)) = local_addr.rsplit_once(':')
                    && let Ok(port) = port_str.parse::<u16>()
                    && let Ok(pid) = pid_str.parse::<u32>()
                {
                    results.push((pid, port, addr.to_string()));
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

#[cfg(test)]
mod backend_tests {
    use super::*;
    #[test]
    fn unavailable_process_metadata_never_turns_a_listener_into_free_space() {
        let sys = System::new();
        let users = Users::new_with_refreshed_list();
        let error = enrich_process_info(
            &sys,
            &users,
            424242,
            8080,
            "127.0.0.1".into(),
            &HashMap::new(),
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("424242"));
        assert!(message.contains("127.0.0.1:8080"));
        assert!(message.contains("cannot report ports as free"));
    }

    #[test]
    fn linux_parsers_handle_ipv6_and_multiple_pids() {
        let ss =
            r#"LISTEN 0 128 [::1]:8080 [::]:* users:(("node",pid=12,fd=4),("node",pid=13,fd=4))"#;
        assert_eq!(
            parse_linux_listeners(ss, BackendStrategy::Ss).unwrap(),
            vec![(12, 8080, "[::1]".into()), (13, 8080, "[::1]".into())]
        );
        assert_eq!(
            parse_linux_listeners(
                "tcp6 0 0 :::8080 :::* LISTEN 12/node",
                BackendStrategy::Netstat
            )
            .unwrap(),
            vec![(12, 8080, "::".into())]
        );
    }
    #[test]
    fn process_names_cannot_inject_pids() {
        for name in [
            r#"pid=111,"#,
            r#"node ,pid=111,fd=9"#,
            r#"quote\" ,pid=111,"#,
            r#"slash\\,pid=111,"#,
        ] {
            let row = format!(r#"LISTEN 0 128 127.0.0.1:8080 *:* users:(("{name}",pid=222,fd=3))"#);
            assert_eq!(
                parse_linux_listeners(&row, BackendStrategy::Ss).unwrap(),
                vec![(222, 8080, "127.0.0.1".into())],
                "{row}"
            );
        }
    }
    #[test]
    fn hidden_owners_reject_the_entire_snapshot_instead_of_appearing_free() {
        let known_ss = r#"LISTEN 0 128 127.0.0.1:8000 *:* users:(("node",pid=12,fd=4))"#;
        for row in [
            "LISTEN 0 128 0.0.0.0:9090 *:*",
            "LISTEN 0 128 [::]:9090 [::]:* ",
        ] {
            let error = parse_linux_listeners(&format!("{known_ss}\n{row}"), BackendStrategy::Ss)
                .unwrap_err();
            assert!(error.to_string().contains("9090"));
            assert!(error.to_string().contains("cannot report ports as free"));
        }
        assert!(
            parse_linux_listeners(
                "tcp 0 0 0.0.0.0:9090 0.0.0.0:* LISTEN -",
                BackendStrategy::Netstat
            )
            .is_err()
        );
        assert!(
            parse_linux_listeners("", BackendStrategy::Ss)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn malformed_process_metadata_never_produces_a_partial_pid_list() {
        for users in [
            r#"users:(("node",pid=222oops,fd=3))"#,
            r#"users:(("node",pid=0,fd=3))"#,
            r#"users:(("node",pid=222,fd=3),("broken",pid=))"#,
            r#"users:(("node",pid=222,fd=3)),pid=111,"#,
            r#"users:(("unterminated,pid=111,fd=3))"#,
        ] {
            let row = format!("LISTEN 0 128 127.0.0.1:8080 *:* {users}");
            assert!(
                parse_linux_listeners(&row, BackendStrategy::Ss).is_err(),
                "{row}"
            );
        }
    }
}
