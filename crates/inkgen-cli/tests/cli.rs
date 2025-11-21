use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn cli_command() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("inkgen"))
}

#[test]
fn config_init_creates_file() {
    let dir = tempdir().expect("temp dir");
    let config_path = dir.path().join("inkgen.toml");

    cli_command()
        .current_dir(dir.path())
        .args(["config", "init", "--path", config_path.to_str().unwrap()])
        .assert()
        .success();

    assert!(config_path.exists(), "config file should be created");
}

fn write_default_config(dir: &Path) -> std::path::PathBuf {
    let config = dir.join("inkgen.toml");
    fs::write(
        &config,
        r#"
[[packages]]
name = "hl7.fhir.r4.core"
version = "4.0.1"

[tree_shaking]
allowed_resources = []
"#,
    )
    .expect("write config");
    config
}

#[test]
fn config_validate_succeeds() {
    let dir = tempdir().expect("temp dir");
    let config = write_default_config(dir.path());

    cli_command()
        .current_dir(dir.path())
        .args(["config", "validate", "--config", config.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn config_completions_output_file() {
    let dir = tempdir().expect("temp dir");
    let destination = dir.path().join("inkgen.bash");

    cli_command()
        .current_dir(dir.path())
        .args([
            "config",
            "completions",
            "bash",
            "--output",
            destination.to_str().unwrap(),
        ])
        .assert()
        .success();

    let contents = fs::read_to_string(&destination).expect("read completions");
    assert!(
        contents.contains("inkgen-cli"),
        "completion file should contain command name"
    );
}

#[test]
fn fetch_dry_run_succeeds() {
    let dir = tempdir().expect("temp dir");
    let config = write_default_config(dir.path());

    cli_command()
        .current_dir(dir.path())
        .args(["fetch", "--config", config.to_str().unwrap(), "--dry-run"])
        .assert()
        .success();
}

#[test]
fn generate_dry_run_succeeds() {
    let dir = tempdir().expect("temp dir");
    let config = write_default_config(dir.path());

    cli_command()
        .current_dir(dir.path())
        .args([
            "generate",
            "typescript",
            "--config",
            config.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .success();
}
