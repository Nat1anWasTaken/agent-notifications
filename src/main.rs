use std::path::PathBuf;

use anyhow::Error;
use clap::{Parser, Subcommand, command};

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
    Codex,
    Claude,
    Init {
        #[command(subcommand)]
        command: Option<InitCommands>,
    },
}

#[derive(Subcommand)]
enum InitCommands {
    Codex {
        #[arg(short, long)]
        config_path: Option<PathBuf>,
    },
    Claude {
        #[arg(short, long)]
        config_path: Option<PathBuf>,
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

    let input = utils::catch_stdin();

    match &cli.command {
        Some(Commands::Codex) => {
            unimplemented!();
        }
        Some(Commands::Claude) => {
            process_claude_input(input, &config)?;
        }
        Some(Commands::Init { command: _ }) => {
            // println!("Subcommand 'init' was used");
            unimplemented!();
        }
        None => {}
    }

    return Ok(());

    // Continued program logic goes here...
}
