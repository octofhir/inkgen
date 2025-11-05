mod cli;
mod commands;
mod config;
mod error;
mod logging;

use clap::Parser;
use std::time::Instant;
use tracing::info;

use cli::{Cli, Commands, GenerateCommands};
use commands::{execute_config_init, execute_config_show, execute_config_set, execute_fetch, execute_generate_typescript};
use error::{CliError, CliResult};
use logging::{setup_logging, log_command_start, log_command_complete, log_command_error};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    // Set up logging based on CLI arguments
    if let Err(e) = setup_logging(cli.verbose, cli.log_level.clone()) {
        eprintln!("Failed to initialize logging: {}", e.user_message());
        std::process::exit(1);
    }
    
    info!("Starting inkgen CLI");
    
    // Get command name for logging
    let command_name = get_command_name(&cli.command);
    let args = std::env::args().collect::<Vec<_>>();
    
    log_command_start(&command_name, &args[1..]);
    let start_time = Instant::now();
    
    // Execute the command and handle errors gracefully
    match execute_command(cli.command).await {
        Ok(()) => {
            let duration = start_time.elapsed();
            log_command_complete(&command_name, duration);
            info!("Command completed successfully in {:.2}s", duration.as_secs_f64());
        }
        Err(e) => {
            let duration = start_time.elapsed();
            log_command_error(&command_name, &e, duration);
            
            // Print user-friendly error message to stderr
            eprintln!("\n❌ {}", e.user_message());
            
            // Exit with appropriate error code
            let exit_code = match &e {
                CliError::InvalidArguments { .. } => 2,
                CliError::FileNotFound { .. } => 3,
                CliError::PermissionDenied { .. } => 4,
                CliError::NetworkError { .. } => 5,
                CliError::ConfigExists { .. } => 6,
                _ => 1,
            };
            
            std::process::exit(exit_code);
        }
    }
}

async fn execute_command(command: Commands) -> CliResult<()> {
    match command {
        Commands::Fetch { package, version, force } => {
            info!("Executing fetch command for package: {}", package);
            execute_fetch(package, version, force).await
        }
        Commands::Generate { language } => {
            info!("Executing generate command");
            execute_generate_command(language).await
        }
        Commands::Config { show, set, init, output, force } => {
            info!("Executing config command");
            execute_config(show, set, init, output, force)
        }
    }
}

async fn execute_generate_command(language: GenerateCommands) -> CliResult<()> {
    match language {
        GenerateCommands::Typescript { config, output, package } => {
            info!("Executing TypeScript generation");
            execute_generate_typescript(config, output, package).await
        }
    }
}

fn execute_config(show: bool, set: Option<String>, init: bool, output: std::path::PathBuf, force: bool) -> CliResult<()> {
    if init {
        execute_config_init(output, force)
    } else if show {
        execute_config_show()
    } else if let Some(key_value) = set {
        execute_config_set(&key_value)
    } else {
        // Default to showing help if no flags are provided
        println!("Usage: inkgen config [--show] [--set key=value] [--init]");
        println!("Use --help for more information.");
        Ok(())
    }
}

/// Get command name for logging purposes
fn get_command_name(command: &Commands) -> String {
    match command {
        Commands::Fetch { .. } => "fetch".to_string(),
        Commands::Generate { language } => match language {
            GenerateCommands::Typescript { .. } => "generate-typescript".to_string(),
        },
        Commands::Config { init: true, .. } => "config-init".to_string(),
        Commands::Config { show: true, .. } => "config-show".to_string(),
        Commands::Config { set: Some(_), .. } => "config-set".to_string(),
        Commands::Config { .. } => "config".to_string(),
    }
}