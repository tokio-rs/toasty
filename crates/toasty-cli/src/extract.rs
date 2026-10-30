//! Schema extraction: build the user's package and read the schema back out
//! of the artifact. The dump constructor in `toasty::schema_dump` fires when
//! `TOASTY_DUMP_SCHEMA` is set — before `main` for a bin, during `dlopen` for
//! a cdylib — writes a JSON [`SchemaDump`] to stdout, and exits.

use crate::cargo::{self, BuildTarget};
use crate::{Flavor, Project};

use anyhow::{Context, Result, bail};
use console::style;
use std::path::Path;
use std::process::{Command, Stdio};
use toasty::schema::db;
use toasty::schema_dump::{
    DUMP_SCHEMA_ENV, DUMP_TABLE_NAME_PREFIX_ENV, SCHEMA_DUMP_VERSION, SchemaDump,
};

/// Builds the selected target of `project` and extracts its schema, lowered
/// for `flavor`.
pub fn extract_schema(project: &Project, flavor: Flavor, bin: Option<&str>) -> Result<db::Schema> {
    // Running the artifact is only safe when `toasty` is linked: its
    // constructor dumps and exits before `main`. Without it the environment
    // variable is inert and the user's program would run.
    if !project.links_toasty {
        bail!(
            "`{}` does not depend on `toasty`, directly or transitively, so it has no schema \
             to extract; select the package that does with `-p <package>`",
            project.package_name
        );
    }

    let target = project.build_target(bin)?;

    // Progress goes to stderr so `migrate snapshot` stdout stays parseable.
    eprintln!(
        "  {} Compiling {}...",
        style("→").cyan(),
        style(&project.package_name).bold()
    );

    let artifact = cargo::build_artifact(
        &project.workspace_root,
        &project.package_name,
        &target,
        &project.features,
    )?;

    let prefix = project.config.migration.table_name_prefix.as_deref();
    let output = run_dumper(&artifact, &target, flavor, prefix)?;
    let schema = parse_dump(&output, &project.package_name)?;

    check_not_empty(&schema, project, &target)?;

    Ok(schema)
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
            let mut cmd = Command::new(artifact);
            cmd.env(DUMP_SCHEMA_ENV, flavor.as_str());
            // Don't let an inherited prefix through; `Toasty.toml` is the
            // only source for it.
            match table_name_prefix {
                Some(prefix) => cmd.env(DUMP_TABLE_NAME_PREFIX_ENV, prefix),
                None => cmd.env_remove(DUMP_TABLE_NAME_PREFIX_ENV),
            };
            cmd
        }
        BuildTarget::Cdylib => {
            // A cdylib has no entry point, and its constructor exits the
            // process that loads it, so the CLI re-execs itself to `dlopen`
            // it in a child. The subcommand sets the environment variable
            // itself: set here, a debug build of the CLI (which links
            // `toasty`) would dump its own empty schema at startup.
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
/// Other constructors in the user's dependency graph may also write to
/// stdout, so every line is tried and unrelated JSON with a `version` field
/// must not mask the real dump. A version mismatch is reported only once no
/// line decoded.
fn parse_dump(output: &[u8], package: &str) -> Result<db::Schema> {
    let text = String::from_utf8_lossy(output);
    let mut mismatched_version = None;
    let mut decode_error = None;

    for value in text
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    {
        let Some(version) = value.get("version").and_then(serde_json::Value::as_u64) else {
            continue;
        };

        if !value
            .get("schema")
            .is_some_and(serde_json::Value::is_object)
        {
            continue;
        }

        if version != u64::from(SCHEMA_DUMP_VERSION) {
            mismatched_version.get_or_insert(version);
            continue;
        }

        match serde_json::from_value::<SchemaDump>(value) {
            Ok(dump) => return Ok(dump.schema),
            Err(err) => decode_error.get_or_insert(err),
        };
    }

    if let Some(version) = mismatched_version {
        bail!(
            "the schema dumper reported dump format version {version}, but this CLI speaks \
             version {SCHEMA_DUMP_VERSION}; update `toasty-cli` or the project's `toasty` \
             dependency so the two match"
        );
    }

    if let Some(err) = decode_error {
        return Err(
            anyhow::Error::new(err).context("failed to decode the schema produced by the dumper")
        );
    }

    // Reachability is not linkage: rustc only links crates the target
    // actually references, so this bin may carry no constructor at all.
    bail!(
        "the schema dumper produced no schema; check that `{package}` references the crate \
         holding the models, and that its `dev` profile does not strip link-time constructors"
    )
}

/// Rejects a schema with no tables: generating from it would `DROP TABLE`
/// everything in the previous snapshot. The usual cause is a bin that links
/// `toasty` but never references the crate holding the models.
fn check_not_empty(schema: &db::Schema, project: &Project, target: &BuildTarget) -> Result<()> {
    if !schema.tables.is_empty() {
        return Ok(());
    }

    let hint = match target {
        BuildTarget::Bin(name) if project.has_lib => format!(
            "the bin target `{name}` was built, but the models may live in the lib target; \
             `--bin <name>` selects a different one"
        ),
        BuildTarget::Bin(name) => {
            format!("check that the bin target `{name}` references the models")
        }
        BuildTarget::Cdylib => "check that the lib target defines models".to_string(),
    };

    bail!(
        "the schema extracted from `{}` has no tables, so generating would drop every \
         existing table; {hint}",
        project.package_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_checked_before_the_payload_is_decoded() {
        // A newer envelope may lay `schema` out differently; decoding it first
        // would turn the version mismatch into a misleading decode error.
        let output = br#"{"version":999,"schema":{"shaped":"differently"}}"#;

        let err = parse_dump(output, "app").unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("version 999"), "{message}");
        assert!(message.contains("toasty-cli"), "{message}");
    }

    #[test]
    fn a_missing_dump_is_reported() {
        let err = parse_dump(b"not a dump\n", "app").unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("produced no schema"), "{message}");
    }

    #[test]
    fn a_foreign_json_line_does_not_mask_the_real_dump() {
        let dump = serde_json::to_string(&SchemaDump {
            version: SCHEMA_DUMP_VERSION,
            schema: db::Schema::default(),
        })
        .unwrap();
        let output = format!("{{\"version\":\"1.0\",\"schema\":\"public\"}}\n{dump}\n");

        assert!(parse_dump(output.as_bytes(), "app").is_ok());
    }

    #[test]
    fn the_dump_is_found_among_other_constructor_output() {
        let dump = serde_json::to_string(&SchemaDump {
            version: SCHEMA_DUMP_VERSION,
            schema: db::Schema::default(),
        })
        .unwrap();
        let output = format!("another ctor said hello\n{{\"unrelated\":true}}\n{dump}\n");

        assert!(parse_dump(output.as_bytes(), "app").is_ok());
    }
}
