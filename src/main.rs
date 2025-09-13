use std::path::PathBuf;

use anyhow::Error;
use clap::{CommandFactory, Parser, Subcommand, command};
use std::sync::OnceLock;
use tracing::{debug, error};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::{
    configuration::{get_config_path, initialize_configuration, reset_configuration},
    processors::{
        claude::input_and_output::process_claude_input,
        codex::input_and_output::process_codex_input,
    },
};

mod configuration;
mod processors;
mod utils;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Process Claude Code hook events and send desktop notifications (You aren't meant to use this directly. It's called by Claude Code)
    Claude,
    /// Process Codex notifications and send desktop notifications (You aren't meant to use this directly. It's called by Codex)
    Codex {
        /// Notification JSON passed by Codex as a single CLI arg. If absent, read stdin.
        notification: Option<String>,
    },
    /// Initialize configuration for agent notifications
    Init {
        #[command(subcommand)]
        command: Option<InitCommands>,
    },
    Reset,
}

#[derive(Subcommand)]
enum InitCommands {
    /// Initialize Claude Code configuration with notification hooks
    Claude {
        /// Path to Claude Code settings.json file (optional)
        claude_config_path: Option<PathBuf>,
    },
    /// Initialize Codex configuration with notification hooks
    Codex {
        /// Path to Codex config.toml file (optional)
        codex_config_path: Option<PathBuf>,
    },
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    init_tracing(cli.debug);

    let config_path = get_config_path().expect("Failed to determine config path");

    if let Some(Commands::Reset) = cli.command {
        match reset_configuration(config_path.as_path()) {
            Ok(_) => println!(
                "Configuration reset to default at {}",
                config_path.display()
            ),
            Err(e) => eprintln!("Failed to reset configuration: {}", e),
        };
        return Ok(());
    }

    let config =
        initialize_configuration(cli.config.clone().unwrap_or(config_path.clone()).as_path())?;

    match &cli.command {
        Some(Commands::Claude) => {
            debug!("processing Claude input from stdin");
            let input = utils::catch_stdin();
            if let Err(e) = process_claude_input(input, &config) {
                error!(error = %e, "failed to process Claude input");
            }
        }
        Some(Commands::Codex { notification }) => {
            let input = match notification {
                Some(s) => s.clone(),
                None => utils::catch_stdin(),
            };
            if let Err(e) = process_codex_input(input, &config) {
                error!(error = %e, "failed to process Codex input");
            }
        }
        Some(Commands::Init { command }) => match command {
            Some(InitCommands::Claude { claude_config_path }) => {
                crate::processors::claude::init::initialize_claude_configuration(
                    claude_config_path,
                )?;
            }
            Some(InitCommands::Codex { codex_config_path }) => {
                crate::processors::codex::init::initialize_codex_configuration(codex_config_path)?;
            }
            None => {
                let mut cmd = Cli::command();
                if let Some(init_cmd) = cmd.find_subcommand_mut("init") {
                    init_cmd.print_help().ok();
                }
            }
        },
        None => {
            let mut cmd = Cli::command();
            cmd.print_help().ok();
        }
        _ => {
            let mut cmd = Cli::command();
            cmd.print_help().ok();
        }
    }

    Ok(())
}

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

fn init_tracing(verbosity: u8) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| match verbosity {
        0 => EnvFilter::new("warn"),
        1 => EnvFilter::new("info"),
        2 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    });

    let log_dir = crate::configuration::get_logs_dir();

    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "anot.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let fmt_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking)
        .with_target(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();
}
