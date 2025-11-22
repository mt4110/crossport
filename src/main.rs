mod cli;
mod config;
mod core;
mod display;
mod ops;
mod tui;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use core::SystemSnapshot;
use std::io::{self, Write};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::load_config(cli.config.as_ref()).unwrap_or_default();

    // Capture system state once
    // For UI, we might capture inside UI loop, but here we capture for CLI commands.
    // If UI command, we can ignore this snapshot or pass it.
    // TUI App::new() calls capture() again. That's fine.
    let snapshot = SystemSnapshot::capture()?;

    match &cli.command {
        Some(Commands::Ui) => {
            tui::run_tui()?;
        }
        Some(Commands::Scan { from, to, json }) => {
            let (cfg_from, cfg_to) = if let Some(range) = &config.scan.default_range {
                let parts: Vec<&str> = range.split('-').collect();
                if parts.len() == 2 {
                    (parts[0].parse().ok(), parts[1].parse().ok())
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            let final_from = from.or(cfg_from).unwrap_or(3000);
            let final_to = to.or(cfg_to).unwrap_or(9999);

            let results = ops::scan_ports(&snapshot, final_from, final_to)?;

            if *json {
                let json_output = serde_json::to_string_pretty(&results)?;
                println!("{}", json_output);
            } else {
                display::print_scan_result(&results);
            }
        }
        Some(Commands::Suggest {
            base_port,
            env,
            key,
        }) => {
            let port = ops::suggest_port(&snapshot, *base_port, 9999)?;
            println!("Suggested port: {}", port);

            if let Some(env_path) = env {
                utils::update_env_file(env_path, key, &port.to_string())?;
            }
        }
        Some(Commands::Kill {
            port,
            dry_run,
            interactive,
            all_users,
            signal,
            force,
        }) => {
            let infos = snapshot.get_process_info(*port);
            if infos.is_empty() {
                println!("No process found on port {}", port);
                return Ok(());
            }

            let _current_uid = nix::unistd::getuid();

            let final_interactive = interactive.or(config.kill.confirm).unwrap_or(true);
            let final_signal = signal.as_deref().or(config.kill.default_signal.as_deref());

            for info in infos {
                display::print_process_info(&info);

                // Check permission (simplified)
                // In a real app we'd check info.uid vs current_uid
                // But ProcessInfo currently stores user as String.
                // We should probably store uid in ProcessInfo or just trust the user string?
                // For v0, let's just warn if not --all-users and it looks like system.

                if !all_users && info.kind == crate::core::ProcessKind::System {
                    println!("Skipping system process (use --all-users to override)");
                    continue;
                }

                if final_interactive {
                    print!("Kill process {}? [y/N] ", info.pid);
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    if input.trim().to_lowercase() != "y" {
                        println!("Skipped.");
                        continue;
                    }
                }

                ops::kill_process(info.pid, final_signal, *force, *dry_run)?;
            }
        }
        None => {
            if cli.ports.is_empty() {
                // Default behavior if no args? Maybe help?
                // Or maybe scan default range?
                // User said: "crossport scan -> default 3000-9999"
                // But "crossport" (no args) -> maybe help.
                use clap::CommandFactory;
                Cli::command().print_help()?;
                return Ok(());
            }

            for port in cli.ports {
                let infos = snapshot.get_process_info(port);
                if infos.is_empty() {
                    println!("Port {}: free", port);
                } else {
                    for info in infos {
                        display::print_process_info(&info);
                    }
                }
            }
        }
    }

    Ok(())
}
