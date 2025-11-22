use crate::core::{ProcessInfo, SystemSnapshot};
use anyhow::{Context, Result};

#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

#[cfg(windows)]
use std::process::Command;
pub fn scan_ports(snapshot: &SystemSnapshot, from: u16, to: u16) -> Result<Vec<ProcessInfo>> {
    let mut final_infos = Vec::new();

    // Iterate over the range and check if we have info in the snapshot
    // Or iterate over snapshot keys and filter by range (more efficient if range is huge)
    // Since range is usually 3000-9999 (7000 ports), iterating range is fine.
    // But iterating snapshot keys is better if we have few processes.

    // Let's iterate snapshot keys for efficiency
    for (&port, infos) in &snapshot.processes_by_port {
        if port >= from && port <= to {
            final_infos.extend(infos.clone());
        }
    }

    // Sort by port
    final_infos.sort_by_key(|i| i.port);
    Ok(final_infos)
}

pub fn suggest_port(_snapshot: &SystemSnapshot, base: u16, max: u16) -> Result<u16> {
    for port in base..=max {
        if std::net::TcpListener::bind(("0.0.0.0", port)).is_ok() {
            return Ok(port);
        }
    }
    anyhow::bail!("No free ports found in range {}-{}", base, max);
}

pub fn kill_process(
    pid: u32,
    _signal_name: Option<&str>,
    force: bool,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would kill PID {}", pid);
        return Ok(());
    }

    // Unix-specific signal handling
    #[cfg(unix)]
    {
        let nix_pid = Pid::from_raw(pid as i32);

        if force {
            signal::kill(nix_pid, Signal::SIGKILL).context("Failed to send SIGKILL")?;
            println!("Sent SIGKILL to {}", pid);
            return Ok(());
        }

        if let Some(sig_name) = _signal_name {
            let sig = match sig_name.to_uppercase().as_str() {
                "INT" => Signal::SIGINT,
                "TERM" => Signal::SIGTERM,
                "KILL" => Signal::SIGKILL,
                _ => anyhow::bail!("Unknown signal: {}", sig_name),
            };
            signal::kill(nix_pid, sig).context("Failed to send signal")?;
            println!("Sent {} to {}", sig_name, pid);
            return Ok(());
        }

        // Default gentle strategy
        if signal::kill(nix_pid, Signal::SIGINT).is_ok() {
            std::thread::sleep(std::time::Duration::from_secs(1));

            if unsafe { libc::kill(pid as i32, 0) } == 0 {
                if signal::kill(nix_pid, Signal::SIGTERM).is_ok() {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    if unsafe { libc::kill(pid as i32, 0) } == 0 {
                        signal::kill(nix_pid, Signal::SIGKILL)
                            .context("Failed to send SIGKILL (final attempt)")?;
                        println!("Process {} did not exit, sent SIGKILL", pid);
                    } else {
                        println!("Process {} exited after SIGTERM", pid);
                    }
                }
            } else {
                println!("Process {} exited after SIGINT", pid);
            }
        } else {
            println!("Process {} not found or already exited", pid);
        }
    }

    // Windows handling (basic)
    #[cfg(windows)]
    {
        // Windows doesn't have signals like Unix. We can only Terminate.
        // sysinfo::Process::kill() sends SIGTERM equivalent (TerminateProcess).
        // We can use Command("taskkill") for more control if needed, but sysinfo is easier.
        // But we don't have the Process object here, only PID.
        // We can use Command("taskkill").

        let mut cmd = Command::new("taskkill");
        cmd.arg("/PID").arg(pid.to_string());

        if force {
            cmd.arg("/F");
        }

        let output = cmd.output().context("Failed to execute taskkill")?;

        if output.status.success() {
            println!("Killed process {}", pid);
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to kill process {}: {}", pid, stderr);
        }
    }

    Ok(())
}
