//! Minimal CLI placeholder for Task 0001 workspace bootstrap.

use anyhow::Result;
use clap::{ArgAction, Parser};
use tracing_subscriber::EnvFilter;

/// Top-level CLI definition that only supports verbosity flags for now.
#[derive(Parser, Debug)]
#[command(author, version, about = "Inkgen FHIR code generation CLI")]
struct Cli {
    /// Increase verbosity (-v, -vv, etc.).
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
}

fn init_tracing(verbosity: u8) -> Result<()> {
    let level = match verbosity {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::builder()
        .with_default_directive(level.parse()?)
        .from_env_lossy();

    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose)?;
    tracing::info!("inkgen-cli bootstrap complete");
    Ok(())
}
