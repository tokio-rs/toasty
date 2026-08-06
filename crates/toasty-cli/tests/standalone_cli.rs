//! End-to-end tests for the standalone `toasty` binary.
//!
//! These build a throwaway package against this checkout's `toasty` and run
//! the real CLI over it. That is the only way to cover what the design turns
//! on: that a constructor contributed by `toasty` survives into the user's own
//! build artifact, and that running that artifact yields the schema.
//!
//! Both extraction paths are covered, because they differ in how the
//! constructor is reached — before `main` for a bin, during `dlopen` for a
//! lib-only package rebuilt as a `cdylib`.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// A generated package that depends on this checkout's `toasty`.
struct Project {
    dir: tempfile::TempDir,
}

impl Project {
    /// Writes a package whose `src/<name>` holds `source`.
    fn new(entry: &str, source: &str) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let toasty = workspace_root().join("crates/toasty");

        std::fs::write(
            dir.path().join("Cargo.toml"),
            format!(
                r#"
                [package]
                name = "extract-fixture"
                version = "0.0.0"
                edition = "2024"

                [dependencies]
                toasty = {{ path = "{}" }}

                [workspace]
                "#,
                toasty.display()
            ),
        )
        .expect("write manifest");

        std::fs::create_dir(dir.path().join("src")).expect("create src");
        std::fs::write(dir.path().join("src").join(entry), source).expect("write source");

        Self { dir }
    }

    /// Runs the CLI in this project and returns its combined output, requiring
    /// success.
    fn toasty(&self, args: &[&str]) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_toasty"))
            .args(args)
            .current_dir(self.dir.path())
            // Share the workspace target directory so the fixture reuses the
            // already-compiled `toasty` rather than rebuilding its dep tree.
            .env("CARGO_TARGET_DIR", workspace_root().join("target"))
            .output()
            .expect("run toasty");

        assert!(
            output.status.success(),
            "toasty {args:?} failed with {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(path))
            .unwrap_or_else(|err| panic!("reading {path}: {err}"))
    }
}

fn workspace_root() -> PathBuf {
    // `crates/toasty-cli` -> repository root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

const MODELS: &str = r#"
    #[derive(Debug, toasty::Model)]
    pub struct Account {
        #[key]
        pub id: i64,
        #[unique]
        pub email: String,
    }
"#;

/// A bin target reaches the constructor before `main`, so the project's own
/// entry point never runs.
#[test]
fn a_bin_project_yields_its_schema_without_running_main() {
    let project = Project::new(
        "main.rs",
        &format!("{MODELS}\nfn main() {{ panic!(\"main must not run\"); }}"),
    );

    project.toasty(&[
        "migrate",
        "generate",
        "--flavor",
        "postgresql",
        "--name",
        "init",
    ]);

    let sql = project.read("toasty/migrations/0000_init.sql");
    assert!(sql.contains(r#"CREATE TABLE "accounts""#), "{sql}");
    assert!(sql.contains(r#""email" TEXT NOT NULL"#), "{sql}");
}

/// A lib-only project has no entry point at all. This is the path that
/// rebuilds it as a `cdylib` and reaches the constructor through `dlopen`.
#[test]
fn a_lib_only_project_yields_its_schema_through_the_cdylib_path() {
    let project = Project::new("lib.rs", MODELS);

    project.toasty(&[
        "migrate", "generate", "--flavor", "sqlite", "--name", "init",
    ]);

    let sql = project.read("toasty/migrations/0000_init.sql");
    assert!(sql.contains(r#"CREATE TABLE "accounts""#), "{sql}");
}

/// The flavor decides the DDL, and it is chosen without a database to connect
/// to — the whole reason generation is keyed by dialect rather than driver.
#[test]
fn the_flavor_selects_the_dialect() {
    let project = Project::new("lib.rs", MODELS);

    let postgres = project.toasty(&["migrate", "snapshot", "--flavor", "postgresql"]);
    let mysql = project.toasty(&["migrate", "snapshot", "--flavor", "mysql"]);

    // MySQL needs a length on an indexed string column; PostgreSQL does not.
    assert!(postgres.contains("Text"), "{postgres}");
    assert!(mysql.contains("VarChar"), "{mysql}");
}

/// Generating twice with no source change must not invent an empty migration.
#[test]
fn an_unchanged_schema_generates_nothing_the_second_time() {
    let project = Project::new("lib.rs", MODELS);

    project.toasty(&[
        "migrate", "generate", "--flavor", "sqlite", "--name", "init",
    ]);
    let second = project.toasty(&["migrate", "generate", "--flavor", "sqlite"]);

    assert!(second.contains("matches the previous snapshot"), "{second}");
    assert!(
        !project
            .dir
            .path()
            .join("toasty/migrations/0001_migration.sql")
            .exists(),
        "a second migration file should not have been written"
    );
}
