//! End-to-end tests for the standalone `toasty` binary.
//!
//! Each test scaffolds a Cargo package in a temp directory that path-depends
//! on the workspace's `toasty`, then runs the `toasty` binary against it.
//! `CARGO_TARGET_DIR` points at the workspace target directory so dependency
//! builds are shared with the workspace's own cache.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Scaffolds a package using Toasty. `bin` selects a bin package (schema
/// extracted by running the built binary) or a lib-only package (schema
/// extracted by building the lib as a cdylib and loading it).
fn scaffold_project(dir: &Path, bin: bool) {
    let root = workspace_root();

    fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "cli-e2e"
version = "0.0.0"
edition = "2024"

[dependencies]
toasty = {{ path = {toasty_path:?} }}

[workspace]

# Match the Toasty workspace profile so shared dependency builds are reused.
[profile.dev]
debug = "line-tables-only"
"#,
            toasty_path = root.join("crates/toasty"),
        ),
    )
    .unwrap();

    // Pin dependency versions to the workspace's lockfile when there is one;
    // extra entries are pruned by cargo.
    let _ = fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock"));

    let model = r#"
#[derive(Debug, toasty::Model)]
pub struct User {
    #[key]
    #[auto]
    pub id: i64,

    #[unique]
    pub email: String,
}
"#;

    fs::create_dir_all(dir.join("src")).unwrap();
    if bin {
        fs::write(
            dir.join("src/main.rs"),
            format!("{model}\nfn main() {{ panic!(\"user main must not run during schema extraction\"); }}"),
        )
        .unwrap();
    } else {
        fs::write(dir.join("src/lib.rs"), model).unwrap();
    }
}

fn toasty(project: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_toasty"))
        .args(args)
        .current_dir(project)
        .env("CARGO_TARGET_DIR", workspace_root().join("target"))
        .output()
        .unwrap()
}

fn assert_success(output: &Output) -> String {
    assert!(
        output.status.success(),
        "command failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn generate_and_apply_bin_package() {
    let dir = tempfile::tempdir().unwrap();
    scaffold_project(dir.path(), true);

    let output = toasty(
        dir.path(),
        &[
            "migrate", "generate", "--flavor", "sqlite", "--name", "init",
        ],
    );
    let stdout = assert_success(&output);
    assert!(stdout.contains("0000_init.sql"), "{stdout}");

    let sql = fs::read_to_string(dir.path().join("toasty/migrations/0000_init.sql")).unwrap();
    assert!(sql.contains("CREATE TABLE \"users\""), "{sql}");
    assert!(
        dir.path()
            .join("toasty/snapshots/0000_snapshot.toml")
            .is_file()
    );
    assert!(dir.path().join("toasty/history.toml").is_file());
    assert!(dir.path().join("Toasty.toml").is_file());

    // A second generate detects no schema changes.
    let output = toasty(dir.path(), &["migrate", "generate", "--flavor", "sqlite"]);
    let stdout = assert_success(&output);
    assert!(stdout.contains("No migration needed"), "{stdout}");
    assert!(
        !dir.path()
            .join("toasty/migrations/0001_migration.sql")
            .exists()
    );

    // Apply the migration to a SQLite database, then confirm idempotency.
    let url = format!("sqlite:{}", dir.path().join("app.db").display());
    let output = toasty(dir.path(), &["migrate", "apply", "--url", &url]);
    let stdout = assert_success(&output);
    assert!(stdout.contains("Applied: 0000_init.sql"), "{stdout}");
    assert!(dir.path().join("app.db").is_file());

    let output = toasty(dir.path(), &["migrate", "apply", "--url", &url]);
    let stdout = assert_success(&output);
    assert!(stdout.contains("already applied"), "{stdout}");
}

#[test]
fn generate_lib_package_via_cdylib() {
    let dir = tempfile::tempdir().unwrap();
    scaffold_project(dir.path(), false);

    let output = toasty(
        dir.path(),
        &[
            "migrate",
            "generate",
            "--flavor",
            "postgresql",
            "--name",
            "init",
        ],
    );
    let stdout = assert_success(&output);
    assert!(stdout.contains("0000_init.sql"), "{stdout}");

    let sql = fs::read_to_string(dir.path().join("toasty/migrations/0000_init.sql")).unwrap();
    assert!(sql.contains("CREATE TABLE \"users\""), "{sql}");
    assert!(sql.contains("BIGINT"), "{sql}");
}

#[test]
fn generate_requires_flavor() {
    let dir = tempfile::tempdir().unwrap();
    scaffold_project(dir.path(), true);

    let output = toasty(dir.path(), &["migrate", "generate"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--flavor"), "{stderr}");
}

#[test]
fn flavor_from_toasty_toml() {
    let dir = tempfile::tempdir().unwrap();
    scaffold_project(dir.path(), true);
    fs::write(
        dir.path().join("Toasty.toml"),
        "[migration]\npath = \"toasty\"\nprefix_style = \"Sequential\"\nflavor = \"sqlite\"\n",
    )
    .unwrap();

    let output = toasty(dir.path(), &["migrate", "generate", "--name", "init"]);
    let stdout = assert_success(&output);
    assert!(stdout.contains("0000_init.sql"), "{stdout}");
}
