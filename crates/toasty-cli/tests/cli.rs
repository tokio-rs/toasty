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

const MODEL: &str = r#"
#[derive(Debug, toasty::Model)]
pub struct User {
    #[key]
    #[auto]
    pub id: i64,

    #[unique]
    pub email: String,
}
"#;

/// Renders a manifest for a package depending on the workspace's `toasty`.
///
/// `extra` is spliced in after that dependency, so it can add further
/// dependencies or open a new table such as `[lib]`.
fn manifest(name: &str, extra: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.0.0"
edition = "2024"

[dependencies]
toasty = {{ path = {toasty_path:?} }}
{extra}

[workspace]

# Match the Toasty workspace profile so shared dependency builds are reused.
[profile.dev]
debug = "line-tables-only"
"#,
        toasty_path = workspace_root().join("crates/toasty"),
    )
}

/// Pins dependency versions to the workspace lockfile when there is one;
/// extra entries are pruned by cargo.
fn copy_lockfile(dir: &Path) {
    let _ = fs::copy(workspace_root().join("Cargo.lock"), dir.join("Cargo.lock"));
}

/// Scaffolds a package using Toasty. `bin` selects a bin package (schema
/// extracted by running the built binary) or a lib-only package (schema
/// extracted by building the lib as a cdylib and loading it).
fn scaffold_project(dir: &Path, bin: bool) {
    fs::write(dir.join("Cargo.toml"), manifest("cli-e2e", "")).unwrap();
    copy_lockfile(dir);

    fs::create_dir_all(dir.join("src")).unwrap();
    if bin {
        fs::write(
            dir.join("src/main.rs"),
            format!("{MODEL}\nfn main() {{ panic!(\"user main must not run during schema extraction\"); }}"),
        )
        .unwrap();
    } else {
        fs::write(dir.join("src/lib.rs"), MODEL).unwrap();
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

/// The constructor runs before any user code, so a prefix passed to
/// `Db::builder().table_name_prefix(..)` is invisible to it. `Toasty.toml`
/// carries it instead — otherwise the migration creates tables the
/// application never queries.
fn assert_table_name_prefix_is_applied(bin: bool) {
    let dir = tempfile::tempdir().unwrap();
    scaffold_project(dir.path(), bin);
    fs::write(
        dir.path().join("Toasty.toml"),
        "[migration]\npath = \"toasty\"\nprefix_style = \"Sequential\"\n\
         table_name_prefix = \"svc_\"\n",
    )
    .unwrap();

    let output = toasty(
        dir.path(),
        &[
            "migrate", "generate", "--flavor", "sqlite", "--name", "init",
        ],
    );
    assert_success(&output);

    let sql = fs::read_to_string(dir.path().join("toasty/migrations/0000_init.sql")).unwrap();
    assert!(sql.contains("CREATE TABLE \"svc_users\""), "{sql}");
}

#[test]
fn table_name_prefix_reaches_a_bin_dumper() {
    assert_table_name_prefix_is_applied(true);
}

#[test]
fn table_name_prefix_reaches_a_cdylib_dumper() {
    // The prefix travels as a CLI argument on this path rather than as an
    // environment variable, so it needs its own coverage.
    assert_table_name_prefix_is_applied(false);
}

#[test]
fn schema_is_extracted_through_a_transitive_toasty_dependency() {
    // The `models` crate paired with a `server` binary is the layout the guide
    // recommends: `server` reaches `toasty` only through `models`, but the
    // constructor is linked into the binary all the same.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"models\", \"server\"]\nresolver = \"3\"\n\n\
         [profile.dev]\ndebug = \"line-tables-only\"\n",
    )
    .unwrap();
    copy_lockfile(root);

    fs::create_dir_all(root.join("models/src")).unwrap();
    let models_manifest = manifest("models", "");
    fs::write(
        root.join("models/Cargo.toml"),
        // The member manifests must not each declare their own `[workspace]`.
        models_manifest.replace("[workspace]\n", ""),
    )
    .unwrap();
    fs::write(root.join("models/src/lib.rs"), MODEL).unwrap();

    fs::create_dir_all(root.join("server/src")).unwrap();
    fs::write(
        root.join("server/Cargo.toml"),
        "[package]\nname = \"server\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nmodels = { path = \"../models\" }\n",
    )
    .unwrap();
    fs::write(
        root.join("server/src/main.rs"),
        "fn main() { let _ = std::any::type_name::<models::User>(); }\n",
    )
    .unwrap();

    let output = toasty(
        root,
        &[
            "-p", "server", "migrate", "generate", "--flavor", "sqlite", "--name", "init",
        ],
    );
    assert_success(&output);

    let sql = fs::read_to_string(root.join("server/toasty/migrations/0000_init.sql")).unwrap();
    assert!(sql.contains("CREATE TABLE \"users\""), "{sql}");
}

#[test]
fn a_lib_declared_only_as_a_cdylib_is_extractable() {
    // `cargo rustc --crate-type cdylib` overrides whatever the manifest
    // declares, so any linkable crate type works as a dump target.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        manifest("cdylib-only", "\n[lib]\ncrate-type = [\"cdylib\"]\n"),
    )
    .unwrap();
    copy_lockfile(dir.path());
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), MODEL).unwrap();

    let output = toasty(
        dir.path(),
        &[
            "migrate", "generate", "--flavor", "sqlite", "--name", "init",
        ],
    );
    assert_success(&output);

    let sql = fs::read_to_string(dir.path().join("toasty/migrations/0000_init.sql")).unwrap();
    assert!(sql.contains("CREATE TABLE \"users\""), "{sql}");
}

#[test]
fn read_only_commands_do_not_write_to_the_source_tree() {
    // `migrate apply` reads saved migration files and talks to a database; it
    // has no business creating `Toasty.toml`, which would fail outright in a
    // read-only checkout.
    let dir = tempfile::tempdir().unwrap();
    scaffold_project(dir.path(), true);

    let output = toasty(
        dir.path(),
        &[
            "migrate", "generate", "--flavor", "sqlite", "--name", "init",
        ],
    );
    assert_success(&output);

    let config = dir.path().join("Toasty.toml");
    assert!(config.is_file(), "generate should create the config");
    fs::remove_file(&config).unwrap();

    let url = format!("sqlite:{}", dir.path().join("app.db").display());
    let output = toasty(dir.path(), &["migrate", "apply", "--url", &url]);
    assert_success(&output);

    assert!(!config.exists(), "apply must not write Toasty.toml");
}

#[test]
fn a_package_without_toasty_is_never_executed() {
    // Without `toasty` there is no dump constructor, so running the artifact
    // would run the user's `main` — starting a server, writing files, whatever
    // it does. The CLI must refuse before building.
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("main-ran");

    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"no-toasty\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\n\n[workspace]\n\n[profile.dev]\ndebug = \"line-tables-only\"\n",
    )
    .unwrap();
    copy_lockfile(dir.path());
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/main.rs"),
        format!(
            "fn main() {{ std::fs::write({:?}, \"ran\").unwrap(); }}\n",
            marker.display().to_string()
        ),
    )
    .unwrap();

    let output = toasty(
        dir.path(),
        &[
            "migrate", "generate", "--flavor", "sqlite", "--name", "init",
        ],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not depend on `toasty`"), "{stderr}");
    assert!(!marker.exists(), "the user's main() must not be executed");
}

#[test]
fn an_empty_schema_is_rejected_rather_than_dropping_every_table() {
    // A bin that links `toasty` but never references the models dumps zero
    // models. Generating from that writes a DROP for every existing table.
    let dir = tempfile::tempdir().unwrap();
    scaffold_project(dir.path(), false);

    let output = toasty(
        dir.path(),
        &[
            "migrate", "generate", "--flavor", "sqlite", "--name", "init",
        ],
    );
    assert_success(&output);

    fs::create_dir_all(dir.path().join("src/bin")).unwrap();
    fs::write(
        dir.path().join("src/bin/tool.rs"),
        "fn main() { let _ = toasty::Db::builder(); }\n",
    )
    .unwrap();

    let output = toasty(
        dir.path(),
        &[
            "migrate", "generate", "--flavor", "sqlite", "--name", "oops",
        ],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("has no tables"), "{stderr}");
    assert!(
        !dir.path().join("toasty/migrations/0001_oops.sql").exists(),
        "no migration should be written"
    );
}

#[test]
fn snapshot_stdout_is_parseable_toml() {
    let dir = tempfile::tempdir().unwrap();
    scaffold_project(dir.path(), true);

    let output = toasty(dir.path(), &["migrate", "snapshot", "--flavor", "sqlite"]);
    let stdout = assert_success(&output);

    // Headers and build progress belong on stderr so that redirecting stdout
    // to a file yields a usable snapshot.
    let snapshot: toml::Value = toml::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout is not valid TOML: {err}\n---\n{stdout}"));

    let tables = snapshot["schema"]["tables"].as_array().unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0]["name"].as_str(), Some("users"));
}
