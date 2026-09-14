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
    // Extraction runs the built artifact. That is only safe when `toasty` is
    // linked into it: the constructor then dumps and exits before `main`.
    // Without it there is no constructor, the environment variable is inert,
    // and running the artifact would execute the user's program — starting a
    // server, writing files, whatever `main` does.
    if !project.links_toasty {
        bail!(
            "`{}` does not depend on `toasty`, directly or transitively, so it has no schema \
             to extract; select the package that does with `-p <package>`",
            project.package_name
        );
    }

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
    let schema = parse_dump(&output, &project.package_name, project.depends_on_toasty)?;

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
            // The constructor runs before `main`, dumps, and exits; the
            // user's `main` never executes.
            let mut cmd = Command::new(artifact);
            cmd.env(DUMP_SCHEMA_ENV, flavor.as_str());
            // The child inherits this process's environment, so an ambient
            // prefix would silently rename every table. `Toasty.toml` is the
            // only source for it.
            match table_name_prefix {
                Some(prefix) => cmd.env(DUMP_TABLE_NAME_PREFIX_ENV, prefix),
                None => cmd.env_remove(DUMP_TABLE_NAME_PREFIX_ENV),
            };
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
/// Every candidate line is tried rather than just the first one carrying the
/// envelope's keys: an unrelated constructor may print JSON that happens to
/// have a `version` field, and that must not mask the real dump further down.
/// A version mismatch is only reported once no line decoded, so the check
/// still fires for a genuinely newer envelope whose payload this CLI cannot
/// read.
fn parse_dump(output: &[u8], package: &str, direct_dependency: bool) -> Result<db::Schema> {
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

    bail!("{}", no_dump_help(package, direct_dependency))
}

/// Rejects a schema with no tables in it.
///
/// A dump of zero models is indistinguishable from "this project has no
/// models", and generating from it writes a `DROP TABLE` for every table in
/// the previous snapshot. The usual cause is a bin target that links `toasty`
/// but never references the crate holding the models, so the constructor runs
/// with an empty registry.
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
    fn every_optional_column_type_can_be_decoded() {
        // The CLI decodes whatever the user's project dumps. If an optional
        // type feature is off in this crate's `toasty-core`, the variant is
        // missing here and the dump fails with `unknown variant`.
        //
        // Feature unification makes this pass under `cargo test --workspace`
        // regardless; it catches a regression when run as
        // `cargo test -p toasty-cli`.
        for ty in [
            // jiff
            "Timestamp",
            "Zoned",
            "Date",
            "Time",
            "DateTime",
            // rust_decimal / bigdecimal
            "Decimal",
            "BigDecimal",
            // net
            "Cidr",
            "Inet",
            "MacAddr",
            "MacAddr8",
        ] {
            let json = format!("\"{ty}\"");
            let parsed = serde_json::from_str::<toasty_core::stmt::Type>(&json);
            assert!(parsed.is_ok(), "cannot decode `{ty}`: {parsed:?}");
        }
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
    fn a_foreign_json_line_does_not_mask_the_real_dump() {
        // Another constructor printing an object with a `version` field must
        // not abort extraction before the real dump is reached.
        let dump = serde_json::to_string(&SchemaDump {
            version: SCHEMA_DUMP_VERSION,
            schema: db::Schema::default(),
        })
        .unwrap();
        let output = format!("{{\"version\":\"1.0\",\"schema\":\"public\"}}\n{dump}\n");

        assert!(parse_dump(output.as_bytes(), "app", true).is_ok());
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
