mod project;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{ArgAction, Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use inkgen_core::{BaseStructureService, InstallMode, PackageCache, PackageCacheConfig};
use inkgen_typescript::{LanguageGenerator, TypescriptGenerator, TypescriptGeneratorConfig};
use project::{ProjectContext, describe_source, select_requests};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(author, version, about = "Inkgen FHIR code generation CLI")]
struct Cli {
    /// Increase verbosity (-v, -vv, etc.).
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Download and cache configured packages.
    Fetch(FetchArgs),
    /// Generate SDK artifacts.
    Generate(GenerateArgs),
    /// Manage configuration files.
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
struct SharedCacheArgs {
    /// Path to `inkgen.toml`.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Override packages directory.
    #[arg(long)]
    packages_dir: Option<PathBuf>,
    /// Override cache directory.
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    /// Override registry URL.
    #[arg(long)]
    registry_url: Option<String>,
    /// Select a subset of packages (name or name@version).
    #[arg(long = "package", value_name = "PACKAGE")]
    packages: Vec<String>,
    /// Operate without network access (requires cached packages).
    #[arg(long)]
    offline: bool,
    /// Perform a dry run without mutating the workspace.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Debug)]
struct FetchArgs {
    #[command(flatten)]
    shared: SharedCacheArgs,
}

#[derive(Args, Debug)]
struct GenerateArgs {
    #[command(subcommand)]
    command: GenerateSubcommand,
}

#[derive(Subcommand, Debug)]
enum GenerateSubcommand {
    /// Generate TypeScript SDK artifacts.
    Typescript(GenerateTypescriptArgs),
}

#[derive(Args, Debug)]
struct GenerateTypescriptArgs {
    #[command(flatten)]
    shared: SharedCacheArgs,
    /// Output directory (defaults to `target/inkgen/out/typescript`).
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigSubcommand,
}

#[derive(Subcommand, Debug)]
enum ConfigSubcommand {
    /// Create a starter `inkgen.toml`.
    Init(ConfigInitArgs),
    /// Validate an existing `inkgen.toml`.
    Validate(ConfigValidateArgs),
    /// Generate shell completions.
    Completions(ConfigCompletionArgs),
}

#[derive(Args, Debug)]
struct ConfigInitArgs {
    /// Destination path for the manifest.
    #[arg(long, default_value = "inkgen.toml")]
    path: PathBuf,
    /// Overwrite existing manifest.
    #[arg(long)]
    force: bool,
}

#[derive(Args, Debug)]
struct ConfigValidateArgs {
    /// Path to `inkgen.toml`.
    #[arg(long, default_value = "inkgen.toml")]
    config: PathBuf,
}

#[derive(Args, Debug)]
struct ConfigCompletionArgs {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    shell: Shell,
    /// Destination file (stdout when omitted).
    #[arg(long)]
    output: Option<PathBuf>,
}
const DEFAULT_CONFIG_TEMPLATE: &str = r#"# InkGen Configuration File
# By default, all resources from packages are generated

[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"

[tree_shaking]
allowed_resources = ["Patient", "Observation", "Practitioner", "Organization"]

[languages.typescript]
mode = "class_with_builder"
structural_guards = true
naming_convention = "PascalCase"
output_structure = "flat"
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose)?;

    match cli.command {
        Commands::Fetch(args) => fetch_command(args).await,
        Commands::Generate(args) => match args.command {
            GenerateSubcommand::Typescript(ts_args) => generate_typescript(ts_args).await,
        },
        Commands::Config(args) => match args.command {
            ConfigSubcommand::Init(init_args) => config_init(init_args),
            ConfigSubcommand::Validate(validate_args) => config_validate(validate_args),
            ConfigSubcommand::Completions(completion_args) => config_completions(completion_args),
        },
    }
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

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    Ok(())
}

async fn fetch_command(args: FetchArgs) -> Result<()> {
    let context = ProjectContext::load(args.shared.config.clone())?;
    context.validate()?;
    let cache_config = context.build_cache_config(
        args.shared.packages_dir.clone(),
        args.shared.cache_dir.clone(),
        args.shared.registry_url.clone(),
    )?;

    let requests = select_requests(&context.package_requests(), &args.shared.packages);

    if requests.is_empty() {
        warn!("No packages matched the provided filters.");
        return Ok(());
    }

    if args.shared.dry_run {
        info!("dry run: would fetch the following packages:");
        for request in &requests {
            info!("  - {}", request.id);
        }
        return Ok(());
    }

    let cache = build_cache(cache_config).await?;
    let resolver = inkgen_core::PackageResolver::new(Arc::new(cache));
    let mode = if args.shared.offline {
        InstallMode::OfflineOnly
    } else {
        InstallMode::OnlinePreferred
    };

    let descriptors = resolver.ensure_packages(&requests, mode).await?;
    info!("Fetched {} package(s).", descriptors.len());
    for descriptor in descriptors {
        info!(
            "  - {} ({} resources, source: {})",
            descriptor.id,
            descriptor.resource_count,
            describe_source(&descriptor.source)
        );
    }

    Ok(())
}

async fn generate_typescript(args: GenerateTypescriptArgs) -> Result<()> {
    let context = ProjectContext::load(args.shared.config.clone())?;
    context.validate()?;
    let cache_config = context.build_cache_config(
        args.shared.packages_dir.clone(),
        args.shared.cache_dir.clone(),
        args.shared.registry_url.clone(),
    )?;

    let packages = select_requests(&context.package_requests(), &args.shared.packages);

    if packages.is_empty() {
        warn!("No packages matched the provided filters.");
        return Ok(());
    }

    if args.shared.dry_run {
        info!("dry run: would generate TypeScript for packages:");
        for request in &packages {
            info!("  - {}", request.id);
        }
        return Ok(());
    }

    let cache = Arc::new(build_cache(cache_config).await?);
    let resolver = inkgen_core::PackageResolver::new(cache.clone());
    let mode = if args.shared.offline {
        InstallMode::OfflineOnly
    } else {
        InstallMode::OnlinePreferred
    };

    let descriptors = resolver.ensure_packages(&packages, mode).await?;

    let service = Arc::new(BaseStructureService::from_project_config(
        cache,
        context.manifest(),
    ));

    let provider_config = context.manifest().structure_config();
    let default_output = context.default_output_dir();
    let generator_config = TypescriptGeneratorConfig::from_manifest(
        context.typescript_section(),
        default_output,
        args.output.clone(),
    );
    let generator = TypescriptGenerator::new(generator_config);

    for descriptor in descriptors {
        generator
            .generate(&*service, &descriptor, &provider_config)
            .await
            .with_context(|| format!("failed to generate for {}", descriptor.id))?;
    }

    info!(
        "TypeScript generation complete in {}",
        generator.config().output_dir.display()
    );
    Ok(())
}

fn config_init(args: ConfigInitArgs) -> Result<()> {
    let path = if args.path.is_relative() {
        std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(&args.path)
    } else {
        args.path.clone()
    };

    if path.exists() && !args.force {
        anyhow::bail!(
            "configuration file {} already exists (use --force to overwrite)",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    fs::write(&path, DEFAULT_CONFIG_TEMPLATE)
        .with_context(|| format!("failed to write {}", path.display()))?;
    info!("Created {}", path.display());
    Ok(())
}

fn config_validate(args: ConfigValidateArgs) -> Result<()> {
    let context = ProjectContext::load(Some(args.config))?;
    context.validate()?;
    info!("{} is valid.", context.manifest_path().display());
    Ok(())
}

fn config_completions(args: ConfigCompletionArgs) -> Result<()> {
    let mut cmd = Cli::command();
    let shell = args.shell;

    match args.output {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create directory {}", parent.display()))?;
            }

            let mut file = std::fs::File::create(&path)
                .with_context(|| format!("failed to create {}", path.display()))?;
            clap_complete::generate(shell, &mut cmd, "inkgen-cli", &mut file);
            info!(
                "Wrote {} completions to {}",
                shell.to_string().to_lowercase(),
                path.display()
            );
        }
        None => {
            let mut stdout = std::io::stdout();
            clap_complete::generate(shell, &mut cmd, "inkgen-cli", &mut stdout);
        }
    }

    Ok(())
}

async fn build_cache(config: PackageCacheConfig) -> Result<PackageCache> {
    let cache = PackageCache::builder().config(config).build().await?;
    Ok(cache)
}
