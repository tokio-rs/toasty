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
use toasty::schema_dump::{
    DUMP_SCHEMA_ENV, DUMP_TABLE_NAME_PREFIX_ENV, SCHEMA_DUMP_VERSION, SchemaDump,
};
use toasty_core::schema::db;

/// Builds the selected target of `project` and extracts its schema, lowered
/// for `flavor`.
pub fn extract_schema(project: &Project, flavor: Flavor, bin: Option<&str>) -> Result<db::Schema> {
    let target = project.build_target(bin)?;

    // Progress goes to stderr so that `migrate snapshot` can be redirected to
    // a file without the decoration landing in it.
    eprintln!(
        "  {} Compiling {}...",
        style("→").cyan(),
        style(&project.package_name).bold()
    );

    let artifact = cargo::build_artifact(&project.workspace_root, &project.package_name, &target)?;

    let prefix = project.config.migration.table_name_prefix.as_deref();
    let output = run_dumper(&artifact, &target, flavor, prefix)?;
    parse_dump(&output, &project.package_name, project.depends_on_toasty)
}

/// Runs the built artifact so the dump constructor fires, and captures its
/// stdout.
fn run_dumper(
    artifact: &Path,
    target: &BuildTarget,
    flavor: Flavor,
    table_name_prefix: Option<&str>,
) -> Result<Vec<u8>> {
    let mut cmd = match target {
        BuildTarget::Bin(_) => {
            // The constructor runs before `main`, dumps, and exits; the
            // user's `main` never executes.
            let mut cmd = Command::new(artifact);
            cmd.env(DUMP_SCHEMA_ENV, flavor.as_str());
            if let Some(prefix) = table_name_prefix {
                cmd.env(DUMP_TABLE_NAME_PREFIX_ENV, prefix);
            }
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
            if let Some(prefix) = table_name_prefix {
                cmd.args(["--table-name-prefix", prefix]);
            }
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
///
/// The envelope's version is read before the payload is decoded. A future
/// version may lay `schema` out differently, and decoding straight into
/// [`SchemaDump`] would fail on exactly the mismatch the version field exists
/// to report.
fn parse_dump(output: &[u8], package: &str, direct_dependency: bool) -> Result<db::Schema> {
    let text = String::from_utf8_lossy(output);

    let envelope = text
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|value| value.get("version").is_some() && value.get("schema").is_some())
        .with_context(|| no_dump_help(package, direct_dependency))?;

    let version = envelope["version"].as_u64();

    if version != Some(u64::from(SCHEMA_DUMP_VERSION)) {
        bail!(
            "the schema dumper reported dump format version {}, but this CLI speaks version {}; \
             update `toasty-cli` or the project's `toasty` dependency so the two match",
            envelope["version"],
            SCHEMA_DUMP_VERSION
        );
    }

    let dump: SchemaDump = serde_json::from_value(envelope)
        .context("failed to decode the schema produced by the dumper")?;

    Ok(dump.schema)
}

/// Explains why no schema came back, tailored to how `toasty` is reached.
///
/// A transitive `toasty` still carries the constructor — a `models` crate
/// paired with a `server` binary extracts fine — so a missing direct
/// dependency is reported as the most likely cause rather than as a fact.
fn no_dump_help(package: &str, direct_dependency: bool) -> String {
    if direct_dependency {
        format!(
            "the schema dumper produced no schema; check that the `dev` profile of `{package}` \
             does not strip link-time constructors"
        )
    } else {
        format!(
            "the schema dumper produced no schema; `{package}` does not list `toasty` as a \
             direct dependency, so either add it, or select the package that does with \
             `-p <package>`"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_checked_before_the_payload_is_decoded() {
        // A future envelope may lay `schema` out differently. Decoding first
        // would fail on exactly the mismatch the version field exists to
        // report, leaving the user with a misleading dependency error.
        let output = br#"{"version":999,"schema":{"shaped":"differently"}}"#;

        let err = parse_dump(output, "app", true).unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("version 999"), "{message}");
        assert!(message.contains("toasty-cli"), "{message}");
    }

    #[test]
    fn a_missing_dump_names_the_missing_dependency() {
        let err = parse_dump(b"not a dump\n", "app", false).unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("direct dependency"), "{message}");
    }

    #[test]
    fn a_missing_dump_from_a_direct_dependent_points_at_the_build() {
        let err = parse_dump(b"", "app", true).unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("link-time constructors"), "{message}");
    }

    #[test]
    fn the_dump_is_found_among_other_constructor_output() {
        let dump = serde_json::to_string(&SchemaDump {
            version: SCHEMA_DUMP_VERSION,
            schema: db::Schema::default(),
        })
        .unwrap();
        let output = format!("another ctor said hello\n{{\"unrelated\":true}}\n{dump}\n");

        assert!(parse_dump(output.as_bytes(), "app", true).is_ok());
    }
}
