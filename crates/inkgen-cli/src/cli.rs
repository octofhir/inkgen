use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "inkgen")]
#[command(about = "A FHIR code generator")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Enable verbose logging
    #[arg(long, global = true)]
    pub verbose: bool,
    
    /// Set log level (trace, debug, info, warn, error)
    #[arg(long, global = true)]
    pub log_level: Option<String>,
}

#[derive(Subcommand)]
pub enum GenerateCommands {
    /// Generate TypeScript code
    Typescript {
        /// Configuration file path
        #[arg(short, long, default_value = "inkgen.toml")]
        config: PathBuf,
        
        /// Output directory
        #[arg(short, long, default_value = "generated")]
        output: PathBuf,
        
        /// Package to generate from (overrides config)
        #[arg(short, long)]
        package: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum Commands {
    /// Fetch and cache FHIR packages
    Fetch {
        /// Package name (e.g., hl7.fhir.r4.core)
        #[arg(short, long)]
        package: String,
        
        /// Package version (defaults to latest)
        #[arg(short, long)]
        version: Option<String>,
        
        /// Force re-download even if cached
        #[arg(long)]
        force: bool,
    },
    
    /// Generate code from FHIR packages
    Generate {
        #[command(subcommand)]
        language: GenerateCommands,
    },
    
    /// Configuration management
    Config {
        /// Show current configuration
        #[arg(long)]
        show: bool,
        
        /// Set configuration value (key=value format)
        #[arg(long)]
        set: Option<String>,
        
        /// Initialize a new configuration file
        #[arg(long)]
        init: bool,
        
        /// Output path for configuration file (used with --init)
        #[arg(short, long, default_value = "inkgen.toml")]
        output: PathBuf,
        
        /// Overwrite existing file (used with --init)
        #[arg(long)]
        force: bool,
    },
}



