use std::path::PathBuf;

use anyhow::Error;
use clap::{CommandFactory, Parser, Subcommand, command};

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
    unsafe {
        std::env::set_var("RUST_LIB_BACKTRACE", "1");
    }

    let cli = Cli::parse();

    let config_path = get_config_path().expect("Failed to determine config path");

    let config =
        initialize_configuration(cli.config.clone().unwrap_or(config_path.clone()).as_path())?;

    // match cli.debug {
    //     0 => println!("Debug mode is off"),
    //     1 => println!("Debug mode is kind of on"),
    //     2 => println!("Debug mode is on"),
    //     _ => println!("Don't be crazy"),
    // }

    match &cli.command {
        Some(Commands::Claude) => {
            let input = utils::catch_stdin();
            process_claude_input(input, &config).ok();
        }
        Some(Commands::Codex { notification }) => {
            let input = match notification {
                Some(s) => s.clone(),
                None => utils::catch_stdin(),
            };
            process_codex_input(input, &config).ok();
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
        Some(Commands::Reset) => {
            match reset_configuration(config_path.as_path()) {
                Ok(_) => println!(
                    "Configuration reset to default at {}",
                    config_path.display()
                ),
                Err(e) => eprintln!("Failed to reset configuration: {}", e),
            };
        }
        None => {
            let mut cmd = Cli::command();
            cmd.print_help().ok();
        }
    }

    Ok(())
}
