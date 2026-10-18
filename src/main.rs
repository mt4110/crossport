mod cli;
mod config;
mod core;
mod display;
mod exit;
mod history;
mod ops;
mod permissions;
mod tui;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use core::SystemSnapshot;
use std::io::{self, Write};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        // Map error to code
        let code = exit::map_err_to_code(&e);
        code.exit();
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = config::load_config(cli.config.as_ref())?;

    if let Some(color) = config.output.color {
        colored::control::set_override(color);
    }
    let verbose = cli.verbose || config.output.verbose;
    if let Some(Commands::History { json }) = &cli.command {
        let events = history::read(&config.history)?;
        if *json {
            println!("{}", serde_json::to_string_pretty(&events)?);
        } else if events.is_empty() {
            println!("No saved history. Persistence is opt-in via history.enabled.");
        } else {
            for event in events.iter().rev() {
                println!("{}", event.line());
            }
        }
        return Ok(());
    }
    if matches!(&cli.command, Some(Commands::Ui)) {
        return tui::run_tui(config);
    }
    let snapshot = if config.backend.strategy == config::BackendStrategy::Auto {
        SystemSnapshot::capture()?
    } else {
        SystemSnapshot::capture_with(config.backend.strategy)?
    };

    match &cli.command {
        Some(Commands::Ui | Commands::History { .. }) => unreachable!(),
        Some(Commands::Scan {
            from,
            to,
            json,
            watch,
        }) => {
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

            if final_from > final_to {
                anyhow::bail!(
                    "Invalid port range: from ({}) > to ({})",
                    final_from,
                    final_to
                );
            }

            if *watch && *json {
                anyhow::bail!(
                    "Cannot use --watch with --json. Use 'watch -n 1 crossport scan --json' or similar instead."
                );
            }

            if *watch {
                loop {
                    print!("\x1B[2J\x1B[1;1H"); // ANSI clear screen
                    io::stdout().flush()?;

                    // Re-capture
                    let snapshot = SystemSnapshot::capture_with(config.backend.strategy)?;
                    let results = ops::scan_ports(&snapshot, final_from, final_to)?;

                    display::print_scan_result(&results);
                    if verbose {
                        for info in &results {
                            display::print_verbose(info);
                        }
                    }
                    println!("\n(Ctrl+C to exit)");

                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            } else {
                let results = ops::scan_ports(&snapshot, final_from, final_to)?;

                if *json {
                    let json_output = serde_json::to_string_pretty(&results)?;
                    println!("{}", json_output);
                } else {
                    display::print_scan_result(&results);
                    if verbose {
                        for info in &results {
                            display::print_verbose(info);
                        }
                    }
                }
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

            let final_interactive = interactive.or(config.kill.confirm).unwrap_or(true);
            let final_signal = signal.as_deref().or(config.kill.default_signal.as_deref());

            let mut seen = std::collections::HashSet::new();
            for info in infos {
                if !seen.insert(info.pid) {
                    continue;
                }
                display::print_process_info(&info);

                if verbose {
                    display::print_verbose(&info);
                }
                if let Err(error) = permissions::ensure_owned(&info, *all_users) {
                    save_event(
                        &config.history,
                        history::Event::new(
                            info.pid,
                            info.port,
                            history::Action::Stop,
                            history::Outcome::Failed,
                        ),
                    );
                    return Err(error);
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

                let result = permissions::ensure_owned(&info, *all_users)
                    .and_then(|_| ops::kill_process(info.pid, final_signal, *force, *dry_run));
                let outcome = if result.is_err() {
                    history::Outcome::Failed
                } else if *dry_run {
                    history::Outcome::DryRun
                } else {
                    history::Outcome::Success
                };
                save_event(
                    &config.history,
                    history::Event::new(info.pid, info.port, history::Action::Stop, outcome),
                );
                result?;
            }
        }
        None => {
            let mut ports = cli.ports.clone();
            if ports.is_empty()
                && let Some(port) = config.general.default_port
            {
                ports.push(port);
            }
            if ports.is_empty() {
                // Default behavior if no args? Maybe help?
                // Or maybe scan default range?
                // User said: "crossport scan -> default 3000-9999"
                // But "crossport" (no args) -> maybe help.
                use clap::CommandFactory;
                Cli::command().print_help()?;
                return Ok(());
            }

            for port in ports {
                let infos = snapshot.get_process_info(port);
                if infos.is_empty() {
                    println!("Port {}: free", port);
                } else {
                    for info in infos {
                        display::print_process_info(&info);
                        if verbose {
                            display::print_verbose(&info);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn save_event(config: &config::HistoryConfig, event: history::Event) {
    if let Err(error) = history::record(config, &event) {
        eprintln!("History save failed (operation is not retried): {error}");
    }
}
