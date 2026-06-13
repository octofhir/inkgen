mod diff;
mod project;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{ArgAction, Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use inkgen_core::{
    BackendRegistry, BaseStructureService, DependencyAnalyzer, FilterMode, InstallMode,
    LanguageGenerator, PackageCache, PackageCacheConfig, StructureDefinitionProvider,
    StructureFilter, StructureKind,
};
use inkgen_typescript::{TypeRegistry, TypescriptGenerator, TypescriptGeneratorConfig};
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
    /// Inspect intermediate representation and resolved structures.
    Inspect(InspectArgs),
    /// Explain how a StructureDefinition maps to generated code, and why.
    Explain(ExplainArgs),
    /// List available code generation backends.
    Backends,
    /// Manage configuration files.
    Config(ConfigArgs),
    /// Compare two generated output directories.
    Diff(DiffArgs),
    /// Verify environment and dependencies are properly configured.
    Doctor,
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
    /// Generate Rust SDK artifacts (reference backend on the PackageIr contract).
    Rust(GenerateRustArgs),
}

#[derive(Args, Debug)]
struct GenerateRustArgs {
    #[command(flatten)]
    shared: SharedCacheArgs,
    /// Output directory (defaults to `generated/rust`).
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct GenerateTypescriptArgs {
    #[command(flatten)]
    shared: SharedCacheArgs,
    /// Output directory (defaults to `generated/`).
    #[arg(long)]
    output: Option<PathBuf>,
    /// Write a generation report and debug artifacts to `.inkgen/debug/`.
    #[arg(long)]
    report: bool,
    /// Verify determinism: regenerate into a temp dir and fail if it differs
    /// from the existing output directory (does not modify the output).
    #[arg(long)]
    verify: bool,
}

#[derive(Args, Debug)]
struct InspectArgs {
    #[command(subcommand)]
    command: InspectSubcommand,
}

#[derive(Subcommand, Debug)]
enum InspectSubcommand {
    /// Resolve a canonical URL and print its IR as JSON.
    Ir(InspectIrArgs),
}

#[derive(Args, Debug)]
struct InspectIrArgs {
    #[command(flatten)]
    shared: SharedCacheArgs,
    /// Canonical URL of the StructureDefinition to inspect.
    #[arg(value_name = "CANONICAL")]
    canonical: String,
    /// Emit compact (single-line) JSON instead of pretty-printed.
    #[arg(long)]
    compact: bool,
}

#[derive(Args, Debug)]
struct ExplainArgs {
    #[command(flatten)]
    shared: SharedCacheArgs,
    /// Canonical URL of the StructureDefinition to explain.
    #[arg(value_name = "CANONICAL")]
    canonical: String,
    /// Only explain elements whose path contains this substring.
    #[arg(long)]
    element: Option<String>,
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

#[derive(Args, Debug)]
struct DiffArgs {
    /// Directory containing the original/baseline files.
    #[arg(long)]
    old: PathBuf,
    /// Directory containing the new/generated files.
    #[arg(long)]
    new: PathBuf,
    /// Optional file extension filter (e.g., ".ts").
    #[arg(long)]
    extension: Option<String>,
    /// Number of context lines to show around differences.
    #[arg(long, default_value = "3")]
    context: usize,
}
const DEFAULT_CONFIG_TEMPLATE: &str = r#"# InkGen Configuration File

[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"

[languages.typescript]
# All TypeScript features are enabled by default.
# Uncomment the options below to customize or opt out.
# output_dir = "generated"
# structural_guards = false        # Disable runtime type guards
# generate_profiles = false        # Skip profile classes/interfaces
# generate_valuesets = false       # Skip ValueSet helpers
# profile_classes = false          # Generate interfaces instead of classes for profiles
# zod_schemas = false              # Skip Zod schema generation
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose)?;

    match cli.command {
        Commands::Fetch(args) => fetch_command(args).await,
        Commands::Generate(args) => match args.command {
            GenerateSubcommand::Typescript(ts_args) => generate_typescript(ts_args).await,
            GenerateSubcommand::Rust(rust_args) => generate_rust(rust_args).await,
        },
        Commands::Inspect(args) => match args.command {
            InspectSubcommand::Ir(ir_args) => inspect_ir(ir_args).await,
        },
        Commands::Explain(args) => explain_command(args).await,
        Commands::Backends => list_backends_command(),
        Commands::Config(args) => match args.command {
            ConfigSubcommand::Init(init_args) => config_init(init_args),
            ConfigSubcommand::Validate(validate_args) => config_validate(validate_args),
            ConfigSubcommand::Completions(completion_args) => config_completions(completion_args),
        },
        Commands::Diff(args) => diff_command(args),
        Commands::Doctor => doctor_command(),
    }
}

fn init_tracing(verbosity: u8) -> Result<()> {
    let level = match verbosity {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    // Configure logging with reduced verbosity for canonical manager
    // Filter out verbose logs from octofhir-canonical-manager crate:
    // - "Resolving canonical URL" messages
    // - "search.execute" span logs
    use tracing_subscriber::layer::{Layer, SubscriberExt};
    use tracing_subscriber::util::SubscriberInitExt;

    let filter = EnvFilter::builder()
        .with_default_directive(level.parse()?)
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                // Logs go to stderr so command output (e.g. `inspect ir` JSON)
                // stays clean on stdout for piping.
                .with_writer(std::io::stderr)
                .with_target(false)
                // Filter out verbose canonical manager logs
                .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
                    let target = metadata.target();
                    let name = metadata.name();

                    // Exclude canonical resolution logs
                    if target.contains("canonical") && name.contains("resolve") {
                        return false;
                    }

                    // Exclude search execution logs (both span and event)
                    if name.contains("search") || target.contains("search") {
                        return false;
                    }

                    true
                })),
        )
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
        cache.clone(),
        context.manifest(),
    ));

    let provider_config = context.manifest().structure_config();
    let default_output = context.default_output_dir();

    // Build package folder mapping and filter settings from config
    let mut package_folders = std::collections::HashMap::new();
    let mut package_filters = std::collections::HashMap::new();
    let mut needs_dependency_analysis = false;

    for entry in &context.manifest().packages {
        let package_id = inkgen_core::PackageId {
            name: entry.name.clone(),
            version: entry.version.clone(),
        };
        package_folders.insert(package_id.clone(), entry.folder_name());
        package_filters.insert(package_id.clone(), entry.clone());

        // Check if any package uses Dependencies filter mode
        if entry.filter == FilterMode::Dependencies {
            needs_dependency_analysis = true;
        }
    }

    // Three-phase dependency analysis (if needed)
    let dependency_analyzer = if needs_dependency_analysis {
        info!("Performing three-phase dependency analysis across all packages...");
        let mut analyzer = DependencyAnalyzer::new();

        // Phase 1: Register all packages and their resources
        info!("Phase 1: Registering packages and resources...");
        for descriptor in &descriptors {
            let filter = StructureFilter::from_config(&provider_config);
            let summaries = service.list_structures(&filter).await?;

            let package_key = format!("{}", descriptor.id);
            let urls: Vec<String> = summaries
                .iter()
                .filter(|s| s.package == descriptor.id)
                .map(|s| s.canonical_url.clone())
                .collect();

            analyzer.register_package(&package_key, urls);
            info!(
                "  Registered {} with {} resources",
                package_key,
                summaries.len()
            );
        }

        // Phase 2: Analyze dependencies across all packages
        info!("Phase 2: Analyzing cross-package dependencies...");
        for descriptor in &descriptors {
            let filter = StructureFilter::from_config(&provider_config);
            let summaries = service.list_structures(&filter).await?;
            let package_key = format!("{}", descriptor.id);

            for summary in summaries.iter().filter(|s| s.package == descriptor.id) {
                // Only analyze BaseResource and ComplexType structures
                if summary.kind == StructureKind::BaseResource
                    || summary.kind == StructureKind::ComplexType
                    || summary.kind == StructureKind::Logical
                {
                    let structure = service
                        .load_structure(&summary.canonical_url)
                        .await
                        .with_context(|| format!("failed to load {}", summary.canonical_url))?;
                    analyzer.analyze(&structure, &package_key);
                }
            }
        }

        // Log dependency statistics
        let (total_packages, total_resources, total_deps) = analyzer.statistics();
        info!(
            "Dependency analysis complete: {} packages, {} resources, {} cross-package dependencies",
            total_packages, total_resources, total_deps
        );

        Some(analyzer)
    } else {
        None
    };

    // Build global type registry for cross-package imports
    info!("Building global type registry for cross-package imports...");
    let mut type_registry = TypeRegistry::new();

    for descriptor in &descriptors {
        let filter = StructureFilter::from_config(&provider_config);
        let summaries = service.list_structures(&filter).await?;
        let package_folder = package_folders
            .get(&descriptor.id)
            .cloned()
            .unwrap_or_else(|| inkgen_core::sanitize_package_name(&descriptor.id.name));

        let mut used_stems: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for summary in summaries.iter().filter(|s| s.package == descriptor.id) {
            // Only register BaseResource, ComplexType, PrimitiveType, and Logical structures
            if summary.kind == StructureKind::BaseResource
                || summary.kind == StructureKind::ComplexType
                || summary.kind == StructureKind::PrimitiveType
                || summary.kind == StructureKind::Logical
            {
                let structure = service
                    .load_structure(&summary.canonical_url)
                    .await
                    .with_context(|| format!("failed to load {}", summary.canonical_url))?;

                // Generate type name using pascal case (matching TypeScript generator logic)
                let raw_name = summary.type_code.as_deref().unwrap_or(&structure.id);
                let type_name = to_pascal_case(raw_name);

                // Generate file stem using snake case (matching TypeScript generator logic)
                let mut stem = to_snake_case(&structure.id)
                    .replace('_', "-")
                    .to_ascii_lowercase();

                let counter = used_stems.entry(stem.clone()).or_insert(0);
                *counter += 1;
                if *counter > 1 {
                    stem = format!("{stem}_{}", counter);
                }

                type_registry.register(type_name, package_folder.clone(), stem);
            }
        }
    }

    info!(
        "Type registry built with types from {} packages",
        descriptors.len()
    );

    let mut generator_config = TypescriptGeneratorConfig::from_manifest(
        context.typescript_section(),
        default_output,
        args.output.clone(),
        package_folders,
        package_filters,
        dependency_analyzer,
        Some(type_registry),
        Some(cache.clone()),
    );

    // --verify: regenerate into a temp dir and diff against the real output,
    // without touching it. Hold the TempDir guard until after the comparison.
    let real_output = generator_config.output_dir.clone();
    let verify_tmp = if args.verify {
        let td = tempfile::tempdir().context("failed to create temp dir for --verify")?;
        generator_config.output_dir = td.path().to_path_buf();
        Some(td)
    } else {
        None
    };

    let mut generator = TypescriptGenerator::new(generator_config);

    // Capture package summary for the optional report before consuming descriptors.
    let report_packages: Vec<(String, usize)> = descriptors
        .iter()
        .map(|d| (d.id.to_string(), d.resource_count))
        .collect();

    // Phase 3: Generate with filtering
    info!("Phase 3: Generating TypeScript code with filtering...");
    let started = std::time::Instant::now();
    for descriptor in descriptors {
        // Build the PackageIr once per package — the single source of truth the
        // backend reads from instead of the provider/cache.
        let ir = inkgen_core::build_package_ir(
            &*service,
            cache.as_ref(),
            &descriptor,
            &provider_config,
            Some(descriptor.id.version.clone()),
            Vec::new(),
        )
        .await
        .with_context(|| format!("failed to build PackageIr for {}", descriptor.id))?;
        generator.set_package_ir(Some(std::sync::Arc::new(ir)));

        generator
            .generate(&*service, &descriptor, &provider_config)
            .await
            .with_context(|| format!("failed to generate for {}", descriptor.id))?;
    }
    let elapsed = started.elapsed();

    let output_dir = generator.config().output_dir.clone();

    // --verify: compare the freshly generated temp output against the real one.
    if let Some(tmp) = verify_tmp {
        let cfg = diff::DiffConfig::new(real_output.clone(), output_dir.clone());
        let result = diff::diff_directories(&cfg)?;
        let drift = result.files_added + result.files_removed + result.files_changed;
        drop(tmp); // clean up temp dir
        if drift > 0 {
            anyhow::bail!(
                "determinism check FAILED against {}: {} added, {} removed, {} changed. \
                 Regenerate and commit the output, or investigate non-deterministic generation.",
                real_output.display(),
                result.files_added,
                result.files_removed,
                result.files_changed
            );
        }
        info!(
            "Determinism verified: regeneration matches {} ({} files identical)",
            real_output.display(),
            result.files_identical
        );
        return Ok(());
    }

    info!("TypeScript generation complete in {}", output_dir.display());

    if args.report {
        write_generation_report(&output_dir, &report_packages, elapsed)
            .context("failed to write generation report")?;
    }
    Ok(())
}

/// Generate a Rust SDK via the reference backend on the `PackageIr` contract.
///
/// Unlike `generate_typescript` (which still runs the provider-based pipeline),
/// this path builds a `PackageIr` once in core and hands it to a `Backend` that
/// makes zero provider/resolver calls — the proof the IR is language-neutral.
async fn generate_rust(args: GenerateRustArgs) -> Result<()> {
    use inkgen_core::{Backend, build_package_ir};
    use inkgen_rust::{RustGenerator, RustGeneratorConfig};

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
        info!("dry run: would generate Rust for packages:");
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
    let service = BaseStructureService::from_project_config(cache.clone(), context.manifest());
    let provider_config = context.manifest().structure_config();

    let output_dir = args
        .output
        .clone()
        .unwrap_or_else(|| context.default_output_dir().join("rust"));

    let backend = RustGenerator::new(RustGeneratorConfig::new(output_dir.clone()));

    for descriptor in &descriptors {
        // Build the IR once, in core. The backend never touches the provider.
        let ir = build_package_ir(
            &service,
            cache.as_ref(),
            descriptor,
            &provider_config,
            Some(descriptor.id.version.clone()),
            Vec::new(),
        )
        .await
        .with_context(|| format!("failed to build PackageIr for {}", descriptor.id))?;

        let output = backend
            .generate(&ir)
            .map_err(|err| anyhow::anyhow!("rust backend failed: {err}"))?;

        // Core owns writing: deterministic order, create parent dirs.
        fs::create_dir_all(&output_dir)
            .with_context(|| format!("failed to create {}", output_dir.display()))?;
        let mut files: Vec<(&String, &String)> = output.files.iter().collect();
        files.sort_by(|a, b| a.0.cmp(b.0));
        for (rel, content) in files {
            let path = output_dir.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, content)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        info!(
            "Rust backend: wrote {} file(s) for {} to {}",
            output.len(),
            descriptor.id,
            output_dir.display()
        );
    }

    Ok(())
}

/// Write a generation report and a generated-file map to `.inkgen/debug/` in the
/// current working directory (the call site) — never the Rust `target/` dir.
fn write_generation_report(
    output_dir: &std::path::Path,
    packages: &[(String, usize)],
    elapsed: std::time::Duration,
) -> Result<()> {
    let debug_dir = PathBuf::from(".inkgen").join("debug");
    fs::create_dir_all(&debug_dir)
        .with_context(|| format!("failed to create {}", debug_dir.display()))?;

    // Collect generated files (relative path + byte size), sorted for determinism.
    let mut files: Vec<(String, u64)> = Vec::new();
    collect_files(output_dir, output_dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let total_bytes: u64 = files.iter().map(|(_, size)| size).sum();

    // generated-file-map.json
    let file_map: Vec<serde_json::Value> = files
        .iter()
        .map(|(path, size)| serde_json::json!({ "path": path, "bytes": size }))
        .collect();
    let file_map_path = debug_dir.join("generated-file-map.json");
    fs::write(
        &file_map_path,
        serde_json::to_string_pretty(&file_map).context("serialize file map")?,
    )
    .with_context(|| format!("failed to write {}", file_map_path.display()))?;

    // report.md
    let mut report = String::new();
    report.push_str("# InkGen generation report\n\n");
    report.push_str(&format!("- Output directory: `{}`\n", output_dir.display()));
    report.push_str(&format!(
        "- Generation time: {:.3}s\n",
        elapsed.as_secs_f64()
    ));
    report.push_str(&format!("- Generated files: {}\n", files.len()));
    report.push_str(&format!("- Total output size: {} bytes\n\n", total_bytes));

    report.push_str("## Input packages\n\n");
    report.push_str("| Package | Resources |\n|---|---|\n");
    for (id, count) in packages {
        report.push_str(&format!("| {} | {} |\n", id, count));
    }
    report.push_str("\n## Generated files\n\n");
    report.push_str("See `generated-file-map.json` for the full list with sizes.\n");

    let report_path = debug_dir.join("report.md");
    fs::write(&report_path, &report)
        .with_context(|| format!("failed to write {}", report_path.display()))?;

    info!(
        "Wrote generation report to {} ({} files)",
        report_path.display(),
        files.len()
    );
    Ok(())
}

/// Recursively collect files under `dir` as (path-relative-to-`root`, size).
fn collect_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<(String, u64)>,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        // Skip dependency/build directories that are not generated artifacts.
        let name = entry.file_name();
        if name == "node_modules" || name == "target" {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push((rel, size));
        }
    }
    Ok(())
}

async fn inspect_ir(args: InspectIrArgs) -> Result<()> {
    let context = ProjectContext::load(args.shared.config.clone())?;
    context.validate()?;
    let cache_config = context.build_cache_config(
        args.shared.packages_dir.clone(),
        args.shared.cache_dir.clone(),
        args.shared.registry_url.clone(),
    )?;

    let packages = select_requests(&context.package_requests(), &args.shared.packages);

    let cache = Arc::new(build_cache(cache_config).await?);
    let resolver = inkgen_core::PackageResolver::new(cache.clone());
    let mode = if args.shared.offline {
        InstallMode::OfflineOnly
    } else {
        InstallMode::OnlinePreferred
    };
    resolver.ensure_packages(&packages, mode).await?;

    let mut service = BaseStructureService::from_project_config(cache.clone(), context.manifest());
    // Allow inspecting profiles (constraint derivations), not just base structures.
    service.config_mut().include_profiles = true;

    let definition = service
        .load_structure(&args.canonical)
        .await
        .with_context(|| format!("failed to resolve {}", args.canonical))?;

    let json = if args.compact {
        serde_json::to_string(&definition)
    } else {
        serde_json::to_string_pretty(&definition)
    }
    .context("failed to serialize IR to JSON")?;

    println!("{json}");
    Ok(())
}

async fn explain_command(args: ExplainArgs) -> Result<()> {
    let context = ProjectContext::load(args.shared.config.clone())?;
    context.validate()?;
    let cache_config = context.build_cache_config(
        args.shared.packages_dir.clone(),
        args.shared.cache_dir.clone(),
        args.shared.registry_url.clone(),
    )?;
    let packages = select_requests(&context.package_requests(), &args.shared.packages);
    let cache = Arc::new(build_cache(cache_config).await?);
    let resolver = inkgen_core::PackageResolver::new(cache.clone());
    let mode = if args.shared.offline {
        InstallMode::OfflineOnly
    } else {
        InstallMode::OnlinePreferred
    };
    resolver.ensure_packages(&packages, mode).await?;

    let mut service = BaseStructureService::from_project_config(cache.clone(), context.manifest());
    service.config_mut().include_profiles = true;

    let def = service
        .load_structure(&args.canonical)
        .await
        .with_context(|| format!("failed to resolve {}", args.canonical))?;

    print_explanation(&def, args.element.as_deref());
    Ok(())
}

/// Render a human-readable explanation of how a resolved structure maps to code.
fn print_explanation(def: &inkgen_core::ir::ResourceDefinition, filter: Option<&str>) {
    println!("# {} — {}", def.name.as_deref().unwrap_or(&def.id), def.url);
    println!("kind: {:?}", def.kind);
    if let Some(t) = &def.fhir_type {
        println!("fhir type: {t}");
    }
    if let Some(d) = def.lineage.derivation {
        println!("derivation: {d:?}");
    }
    if let Some(base) = &def.lineage.base_definition {
        println!("base: {base}");
    }
    println!("\nElements (path  [min..max]  → generated mapping):\n");

    let elements = if !def.flat_elements.is_empty() {
        &def.flat_elements
    } else {
        &def.elements
    };
    for elem in elements {
        explain_element(elem, filter);
    }
}

fn explain_element(elem: &inkgen_core::ir::ElementDefinition, filter: Option<&str>) {
    let matches = filter.is_none_or(|f| elem.path.contains(f));
    if matches {
        use inkgen_core::ir::ElementMax;
        let max = match elem.cardinality.max {
            ElementMax::Finite(n) => n.to_string(),
            ElementMax::Unbounded => "*".to_string(),
        };
        let array = matches!(elem.cardinality.max, ElementMax::Unbounded)
            || matches!(elem.cardinality.max, ElementMax::Finite(n) if n > 1);
        let optional = elem.cardinality.min == 0;

        let mut flags = Vec::new();
        if array {
            flags.push("array".to_string());
        }
        flags.push(if optional { "optional" } else { "required" }.to_string());
        if elem.must_support {
            flags.push("mustSupport".to_string());
        }
        if elem.fixed.is_some() {
            flags.push("fixed".to_string());
        }
        if elem.pattern.is_some() {
            flags.push("pattern".to_string());
        }
        if let Some(sl) = &elem.slicing {
            let discs: Vec<&str> = sl.discriminators.iter().map(|d| d.path.as_str()).collect();
            flags.push(format!("sliced[{}]", discs.join(",")));
        }

        let indent = "  ".repeat(elem.depth);
        println!(
            "{indent}{}  [{}..{}]  {}",
            elem.path,
            elem.cardinality.min,
            max,
            flags.join(", ")
        );
        println!("{indent}    → {}", mapping_rationale(elem));
    }

    for child in &elem.children {
        explain_element(child, filter);
    }
}

/// Explain *why* an element maps to a particular generated type.
fn mapping_rationale(elem: &inkgen_core::ir::ElementDefinition) -> String {
    use inkgen_core::ir::BindingStrength;

    if let Some(cref) = &elem.content_reference {
        return format!("content reference → reuses the type at {cref}");
    }

    if elem.types.len() > 1 {
        let codes: Vec<&str> = elem.types.iter().map(|t| t.code.as_str()).collect();
        return format!(
            "choice type (value[x]) → TypeScript union of {} variants: {}",
            codes.len(),
            codes.join(" | ")
        );
    }

    let type_code = elem
        .types
        .first()
        .map(|t| t.code.as_str())
        .unwrap_or("(none)");

    if let Some(binding) = &elem.binding {
        let vs = binding.value_set.as_deref().unwrap_or("(no ValueSet)");
        return match binding.strength {
            BindingStrength::Required | BindingStrength::Extensible => format!(
                "`{type_code}` with {:?} binding → CLOSED union (enum-like) from ValueSet {vs}",
                binding.strength
            ),
            BindingStrength::Preferred | BindingStrength::Example => format!(
                "`{type_code}` with {:?} binding → OPEN union `… | (string & {{}})` (hint only) from {vs}",
                binding.strength
            ),
        };
    }

    format!("`{type_code}` → mapped to the corresponding TypeScript type")
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

fn diff_command(args: DiffArgs) -> Result<()> {
    let config = diff::DiffConfig::new(args.old, args.new).with_context_lines(args.context);

    let config = match args.extension {
        Some(ext) => config.with_extension_filter(ext),
        None => config,
    };

    let result = diff::diff_directories(&config)?;

    info!(
        "Diff complete: {} added, {} removed, {} changed, {} identical, {} total changes",
        result.files_added,
        result.files_removed,
        result.files_changed,
        result.files_identical,
        result.total_changes
    );

    Ok(())
}

// Helper functions for type name generation (matching TypeScript generator logic)
fn to_pascal_case(value: &str) -> String {
    split_tokens(value)
        .into_iter()
        .map(|token| {
            let mut chars = token.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn to_snake_case(value: &str) -> String {
    split_tokens(value)
        .into_iter()
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

fn split_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut prev_was_lower = false;

    for ch in value.chars() {
        if ch.is_alphanumeric() {
            // Split on camelCase boundary (lowercase followed by uppercase)
            if prev_was_lower && ch.is_ascii_uppercase() && !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            current.push(ch);
            prev_was_lower = ch.is_ascii_lowercase();
        } else if !current.is_empty() {
            tokens.push(current.clone());
            current.clear();
            prev_was_lower = false;
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    if tokens.is_empty() {
        tokens.push(value.to_string());
    }
    tokens
}

/// Create and populate the backend registry with all available backends.
///
/// This function serves as the central registry for all language backends.
/// When adding a new backend, register it here.
fn create_backend_registry(
    generator_config: TypescriptGeneratorConfig,
) -> BackendRegistry<BaseStructureService> {
    let mut registry = BackendRegistry::new();

    // Register TypeScript backend
    registry.register(Box::new(TypescriptGenerator::new(generator_config)));

    // Future: Register other backends here
    // registry.register(Box::new(RustGenerator::new(rust_config)));
    // registry.register(Box::new(PythonGenerator::new(python_config)));
    // registry.register(Box::new(GoGenerator::new(go_config)));

    registry
}

/// List all available code generation backends.
fn list_backends_command() -> Result<()> {
    // Create a minimal registry just for listing (doesn't need real config)
    let temp_config = TypescriptGeneratorConfig::from_manifest(
        None,
        PathBuf::from("."),
        None,
        std::collections::HashMap::new(),
        std::collections::HashMap::new(),
        None,
        None,
        None,
    );

    let registry = create_backend_registry(temp_config);

    println!("Available code generation backends:\n");

    let mut backends = registry.list().to_vec();
    backends.sort();

    for name in backends {
        if let Some(backend) = registry.get(name) {
            println!("  {} - {}", name, backend.description());
            println!("    Version: {}", backend.version());
            println!("    File extension: .{}", backend.file_extension());

            // Check for common features
            let features = [
                "interfaces",
                "classes",
                "builders",
                "validation",
                "serialization",
                "structural-guards",
                "primitives",
                "cross-package-imports",
            ];

            let supported: Vec<_> = features
                .iter()
                .filter(|&&f| backend.supports_feature(f))
                .collect();

            if !supported.is_empty() {
                println!(
                    "    Features: {}",
                    supported.iter().map(|&&f| f).collect::<Vec<_>>().join(", ")
                );
            }
            println!();
        }
    }

    println!("Total backends: {}", registry.len());
    Ok(())
}

fn doctor_command() -> Result<()> {
    println!("InkGen Doctor - Environment Health Check\n");

    let mut all_passed = true;

    // Check 1: Rust version
    print!("✓ Checking Rust toolchain...");
    match std::process::Command::new("rustc")
        .arg("--version")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!(" {}", version.trim());
            } else {
                println!(" FAILED");
                all_passed = false;
            }
        }
        Err(_) => {
            println!(" FAILED - rustc not found");
            println!("  → Install Rust from https://rustup.rs");
            all_passed = false;
        }
    }

    // Check 2: Cargo version
    print!("✓ Checking Cargo...");
    match std::process::Command::new("cargo")
        .arg("--version")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!(" {}", version.trim());
            } else {
                println!(" FAILED");
                all_passed = false;
            }
        }
        Err(_) => {
            println!(" FAILED - cargo not found");
            all_passed = false;
        }
    }

    // Check 3: just command
    print!("✓ Checking 'just' command runner...");
    match std::process::Command::new("just").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!(" {}", version.trim());
            } else {
                println!(" FAILED");
            }
        }
        Err(_) => {
            println!(" NOT INSTALLED (optional)");
            println!("  → Install from https://github.com/casey/just");
        }
    }

    // Check 4: git
    print!("✓ Checking Git...");
    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!(" {}", version.trim());
            } else {
                println!(" FAILED");
                all_passed = false;
            }
        }
        Err(_) => {
            println!(" FAILED - git not found");
            all_passed = false;
        }
    }

    // Check 5: Cache directory
    print!("✓ Checking cache directory...");
    match ProjectContext::load(None) {
        Ok(ctx) => {
            let cache_dir = ctx.default_cache_dir();
            println!(" {}", cache_dir.display());
            match std::fs::metadata(&cache_dir) {
                Ok(_) => println!("  → Readable"),
                Err(_) => {
                    println!("  → WARNING: Cache directory not accessible");
                    // This is not fatal, we can create it
                }
            }
        }
        Err(_) => {
            println!(" ERROR - Could not determine cache directory");
            all_passed = false;
        }
    }

    // Check 6: Workspace config
    print!("✓ Checking for inkgen.toml...");
    if std::path::Path::new("inkgen.toml").exists() {
        println!(" Found");
        match ProjectContext::load(None) {
            Ok(ctx) => {
                println!("  → {} packages configured", ctx.package_requests().len());
            }
            Err(e) => {
                println!("  → WARNING: Config file invalid: {}", e);
            }
        }
    } else {
        println!(" Not found (create with 'inkgen config init')");
    }

    println!("\n{}", "=".repeat(50));
    if all_passed {
        println!("✓ All critical checks passed!");
        println!("\nYour environment is ready for InkGen development.");
        Ok(())
    } else {
        println!("✗ Some checks failed. Please review the messages above.");
        println!("\nFor help, visit: https://github.com/octofhir/inkgen");
        Err(anyhow::anyhow!("Environment validation failed"))
    }
}

#[cfg(test)]
mod explain_tests {
    use super::mapping_rationale;
    use indexmap::IndexMap;
    use inkgen_core::ir::{
        BindingDefinition, BindingStrength, ElementCardinality, ElementDefinition, ElementMax,
        ElementType,
    };

    fn element() -> ElementDefinition {
        ElementDefinition {
            id: "X".to_string(),
            path: "X".to_string(),
            slice_name: None,
            short: None,
            definition: None,
            comment: None,
            requirements: None,
            cardinality: ElementCardinality {
                min: 0,
                max: ElementMax::Finite(1),
            },
            types: Vec::new(),
            content_reference: None,
            binding: None,
            invariants: Vec::new(),
            fixed: None,
            pattern: None,
            default_value: None,
            example_values: Vec::new(),
            must_support: false,
            is_summary: false,
            slicing: None,
            extension: Vec::new(),
            additional_fields: IndexMap::new(),
            children: Vec::new(),
            parent_path: None,
            depth: 0,
            is_backbone: false,
        }
    }

    fn ty(code: &str) -> ElementType {
        ElementType {
            code: code.to_string(),
            profiles: Vec::new(),
            target_profiles: Vec::new(),
            aggregation: Vec::new(),
            versioning: None,
        }
    }

    fn binding(strength: BindingStrength) -> BindingDefinition {
        BindingDefinition {
            strength,
            value_set: Some("http://example.org/vs".to_string()),
            description: None,
            additional: IndexMap::new(),
        }
    }

    #[test]
    fn required_binding_is_closed_union() {
        let mut e = element();
        e.types = vec![ty("code")];
        e.binding = Some(binding(BindingStrength::Required));
        let r = mapping_rationale(&e);
        assert!(r.contains("CLOSED union"), "got: {r}");
        assert!(r.contains("http://example.org/vs"));
    }

    #[test]
    fn extensible_binding_is_closed_union() {
        let mut e = element();
        e.types = vec![ty("code")];
        e.binding = Some(binding(BindingStrength::Extensible));
        assert!(mapping_rationale(&e).contains("CLOSED union"));
    }

    #[test]
    fn preferred_binding_is_open_union() {
        let mut e = element();
        e.types = vec![ty("code")];
        e.binding = Some(binding(BindingStrength::Preferred));
        let r = mapping_rationale(&e);
        assert!(r.contains("OPEN union"), "got: {r}");
    }

    #[test]
    fn choice_type_is_variant_union() {
        let mut e = element();
        e.types = vec![ty("Quantity"), ty("CodeableConcept"), ty("string")];
        let r = mapping_rationale(&e);
        assert!(r.contains("choice type"), "got: {r}");
        assert!(r.contains("3 variants"));
        assert!(r.contains("Quantity | CodeableConcept | string"));
    }

    #[test]
    fn content_reference_is_reported() {
        let mut e = element();
        e.content_reference = Some("#X.item".to_string());
        assert!(mapping_rationale(&e).contains("content reference"));
    }

    #[test]
    fn plain_type_maps_directly() {
        let mut e = element();
        e.types = vec![ty("string")];
        let r = mapping_rationale(&e);
        assert!(r.contains("string"));
        assert!(!r.contains("union"));
    }
}
