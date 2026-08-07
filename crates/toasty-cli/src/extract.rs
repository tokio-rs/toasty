//! Schema extraction: build the user's package and read the schema back out
//! of the built artifact.
//!
//! The dump constructor lives inside `toasty` (see `toasty::schema_dump`) and
//! fires when `TOASTY_DUMP_SCHEMA=<flavor>` is set: before `main` for a bin
//! artifact, or during `dlopen` for a lib built as a `cdylib`. Either way the
//! process writes a JSON [`SchemaDump`] to stdout and exits, and the user's
//! own code never runs.

use crate::cargo::{self, BuildTarget};
use crate::{Flavor, Project};

use anyhow::{Context, Result, bail};
use console::style;
use std::path::Path;
use std::process::{Command, Stdio};
use toasty::schema_dump::{DUMP_SCHEMA_ENV, SCHEMA_DUMP_VERSION, SchemaDump};
use toasty_core::schema::db;

/// Builds the selected target of `project` and extracts its schema, lowered
/// for `flavor`.
pub fn extract_schema(project: &Project, flavor: Flavor, bin: Option<&str>) -> Result<db::Schema> {
    if !project.depends_on_toasty {
        bail!(
            "package `{}` does not depend on `toasty`, so there is no schema to extract",
            project.package_name
        );
    }

    let target = project.build_target(bin)?;

    println!(
        "  {} Compiling {}...",
        style("→").cyan(),
        style(&project.package_name).bold()
    );

    let artifact = cargo::build_artifact(
        project.workspace_root.to_str().unwrap_or("."),
        &project.package_name,
        &target,
    )?;

    let output = run_dumper(&artifact, &target, flavor)?;
    parse_dump(&output, &project.package_name)
}

/// Runs the built artifact so the dump constructor fires, and captures its
/// stdout.
fn run_dumper(artifact: &Path, target: &BuildTarget, flavor: Flavor) -> Result<Vec<u8>> {
    let mut cmd = match target {
        BuildTarget::Bin(_) => {
            // The constructor runs before `main`, dumps, and exits; the
            // user's `main` never executes.
            let mut cmd = Command::new(artifact);
            cmd.env(DUMP_SCHEMA_ENV, flavor.as_str());
            cmd
        }
        BuildTarget::Cdylib => {
            // A cdylib has no entry point, and its constructor exits the
            // process that loads it, so the `dlopen` happens in a child:
            // the CLI re-execs itself with a hidden subcommand. The
            // environment variable is set by the subcommand just before
            // loading — not here — so a debug build of the CLI does not
            // trigger its own dump constructor at startup.
            let current_exe =
                std::env::current_exe().context("failed to locate the `toasty` executable")?;
            let mut cmd = Command::new(current_exe);
            cmd.arg("__load-cdylib")
                .arg(artifact)
                .args(["--flavor", flavor.as_str()]);
            cmd
        }
    };

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("failed to run schema dumper `{}`", artifact.display()))?;

    if !output.status.success() {
        bail!("the schema dumper exited with {}", output.status);
    }

    Ok(output.stdout)
}

/// Parses the dumper's stdout into a schema.
///
/// The dump is a single JSON line, but other link-time constructors in the
/// user's dependency graph may also write to stdout, so each line is tried.
fn parse_dump(output: &[u8], package: &str) -> Result<db::Schema> {
    let text = String::from_utf8_lossy(output);

    let dump = text
        .lines()
        .find_map(|line| serde_json::from_str::<SchemaDump>(line).ok())
        .with_context(|| {
            format!(
                "the schema dumper produced no schema; check that `toasty` is a direct \
                 dependency of `{package}` and that the `dev` profile does not strip \
                 link-time constructors"
            )
        })?;

    if dump.version != SCHEMA_DUMP_VERSION {
        bail!(
            "schema dump version {} does not match this CLI (expected {}); \
             update `toasty-cli` or the project's `toasty` dependency so both use the \
             same major version",
            dump.version,
            SCHEMA_DUMP_VERSION
        );
    }

    Ok(dump.schema)
}
