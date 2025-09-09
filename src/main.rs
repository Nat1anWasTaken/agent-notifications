use std::path::PathBuf;

use anyhow::Error;
use clap::{CommandFactory, Parser, Subcommand, command};

use crate::{
    configuration::{get_config_path, initialize_configuration},
    processors::claude::input_and_output::process_claude_input,
};

mod configuration;
mod processors;
mod utils;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[arg(short, long)]
    reset_config: bool,

    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Process Claude Code hook events and send desktop notifications
    Claude,
    /// Initialize configuration for agent notifications
    Init {
        #[command(subcommand)]
        command: Option<InitCommands>,
    },
}

#[derive(Subcommand)]
enum InitCommands {
    /// Initialize Claude Code configuration with notification hooks
    Claude {
        /// Path to Claude Code settings.json file (optional)
        claude_config_path: Option<PathBuf>,
    },
}

fn main() -> Result<(), Error> {
    unsafe {
        std::env::set_var("RUST_LIB_BACKTRACE", "1");
    }

    let cli = Cli::parse();

    let config = initialize_configuration(
        &cli.config
            .clone()
            .unwrap_or(get_config_path().expect("Config path returned None.")),
        cli.reset_config,
    )?;

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
        Some(Commands::Init { command }) => match command {
            Some(InitCommands::Claude { claude_config_path }) => {
                crate::processors::claude::init::initialize_claude_configuration(
                    claude_config_path,
                )?;
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
    }

    Ok(())
}
