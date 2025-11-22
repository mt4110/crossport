use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Target port numbers (if no subcommand is specified)
    #[arg(value_name = "PORT")]
    pub ports: Vec<u16>,

    /// Explicit configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Verbose output
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan for listening ports
    Scan {
        /// Start of port range (default: 3000 or config)
        #[arg(long)]
        from: Option<u16>,

        /// End of port range (default: 9999 or config)
        #[arg(long)]
        to: Option<u16>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Suggest a free port
    Suggest {
        /// Base port to start searching from
        #[arg(default_value_t = 3000)]
        base_port: u16,

        /// Update .env file with the found port
        #[arg(long)]
        env: Option<PathBuf>,

        /// Key to update in .env file (default: PORT)
        #[arg(long, default_value = "PORT")]
        key: String,
    },

    /// Kill process on specified port
    Kill {
        /// Target port
        port: u16,

        /// Dry run (don't actually kill)
        #[arg(long)]
        dry_run: bool,

        /// Interactive mode (default: true or config)
        #[arg(short, long)]
        interactive: Option<bool>,

        /// Allow killing processes from all users (requires privileges)
        #[arg(long)]
        all_users: bool,

        /// Signal to send (INT, TERM, KILL)
        #[arg(long)]
        signal: Option<String>,

        /// Force kill (SIGKILL)
        #[arg(long)]
        force: bool,
    },

    /// Start interactive TUI mode
    Ui,
}
