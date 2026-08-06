//! Reading a project's schema out of the artifact it already builds.
//!
//! `toasty` contributes a constructor that dumps the schema and exits when
//! `TOASTY_DUMP_SCHEMA` is set (see `toasty::schema::dump`). This module builds
//! the user's own target and runs it with that variable set.
//!
//! A bin is a process to spawn. A `cdylib` has no entry point, so it has to be
//! `dlopen`ed — and the constructor calls `exit`, which would take the CLI down
//! with it. The load therefore happens in a child: the CLI re-execs itself with
//! the hidden `__load-cdylib` subcommand, which exists only for that.

use super::cargo::{self, BuildTarget, Package, TargetFlags};

use anyhow::{Context, Result, bail};
use toasty::schema::{
    app,
    dump::{DUMP_ENV, DUMP_MARKER, Dump},
};

use std::{path::Path, process::Command};

/// Builds `flags`' target package and returns the schema linked into it.
pub fn schema(flags: &TargetFlags) -> Result<app::Schema> {
    let metadata = cargo::Metadata::load(flags)?;
    let package = metadata.target_package(flags)?;

    // The constructor lives in `toasty`. Without it in the graph the build
    // would succeed and produce nothing, which is a confusing way to find out.
    if !metadata.links_toasty(package) {
        bail!(
            "package `{}` does not depend on `toasty`, so it has no schema to \
             extract; add toasty as a dependency, or select another package \
             with `-p <package>`",
            package.name
        );
    }

    let target = BuildTarget::select(package, flags.bin.as_deref())?;
    let artifact = cargo::build(flags, package, &target)?;

    let stdout = run_dumper(&target, &artifact)?;
    parse_dump(&stdout, package)
}

/// Runs the built artifact so its constructor dumps the schema, and returns
/// what it wrote to stdout.
fn run_dumper(target: &BuildTarget, artifact: &Path) -> Result<String> {
    let mut cmd = match target {
        BuildTarget::Bin(_) => {
            let mut cmd = Command::new(artifact);
            // Set only on this child; the variable is never exported into the
            // user's shell.
            cmd.env(DUMP_ENV, "1");
            cmd
        }
        BuildTarget::Cdylib => {
            let toasty = std::env::current_exe()
                .context("could not locate the running `toasty` binary to re-exec")?;
            let mut cmd = Command::new(toasty);
            cmd.arg(LOAD_CDYLIB_SUBCOMMAND).arg(artifact);
            // Deliberately *not* set here. This CLI links `toasty` too, so it
            // carries the dump constructor itself; a child that started with
            // the variable set would dump the CLI's own (empty) schema before
            // reaching the `dlopen`. `load_cdylib` sets it once that risk has
            // passed.
            cmd.env_remove(DUMP_ENV);
            cmd
        }
    };

    let output = cmd
        .output()
        .with_context(|| format!("failed to run `{}`", artifact.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        bail!(
            "the schema dumper exited with {}{}",
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(":\n\n{stderr}")
            }
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Finds the dump payload in the dumper's stdout and decodes it.
///
/// The payload is located by its marker rather than by assuming it is all of
/// stdout: other constructors run before this one, and a project is free to
/// print during static initialization.
fn parse_dump(stdout: &str, package: &Package) -> Result<app::Schema> {
    let Some((_, rest)) = stdout.split_once(DUMP_MARKER) else {
        bail!(
            "`{}` produced no schema dump.\n\n\
             This usually means the `toasty` it links was built without the \
             `schema-dump` feature (it is on by default), so nothing \
             contributed the dump constructor.",
            package.name
        );
    };

    let json = rest.trim_start();
    let dump: Dump =
        serde_json::from_str(json.lines().next().unwrap_or_default()).with_context(|| {
            format!(
                "could not read the schema `{}` dumped. The `toasty` it links \
                 and this CLI may be different versions; reinstall with \
                 `cargo install toasty-cli`.",
                package.name
            )
        })?;

    Ok(dump.schema)
}

/// The hidden subcommand the CLI re-execs to `dlopen` a cdylib.
pub const LOAD_CDYLIB_SUBCOMMAND: &str = "__load-cdylib";

/// Loads `path`, letting the constructors in its initialization image run.
///
/// The library's constructors include the one `toasty` contributes, which —
/// with `TOASTY_DUMP_SCHEMA` set — writes the schema and exits the process
/// from inside the load, so this never returns normally.
///
/// The variable is set here rather than inherited: this process runs its own
/// constructors before `main`, and this CLI links `toasty` too. Setting it now
/// means the only constructor that sees it belongs to the library being
/// loaded.
pub fn load_cdylib(path: &Path) -> Result<()> {
    // SAFETY: `set_var` requires that no other thread is touching the
    // environment. This process was spawned to perform one load and has not
    // started a thread; the dump constructor reads the variable synchronously
    // inside the `Library::new` call below.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(DUMP_ENV, "1");
    }

    // SAFETY: loading a library runs its initializers, which is precisely the
    // point here. The path comes from the build this CLI just ran, and this
    // process exists only to perform the load.
    #[allow(unsafe_code)]
    let library = unsafe { libloading::Library::new(path) }
        .with_context(|| format!("failed to load `{}`", path.display()))?;

    // Reaching here means the library loaded without the constructor firing.
    drop(library);
    bail!(
        "`{}` loaded without dumping a schema; the `toasty` it links was \
         probably built without the `schema-dump` feature",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn package() -> Package {
        serde_json::from_value(json!({
            "name": "app",
            "id": "app 0.1.0",
            "manifest_path": "/ws/app/Cargo.toml",
            "targets": [],
        }))
        .unwrap()
    }

    fn dump_line() -> String {
        json!({ "toasty_version": "0.9.0", "schema": { "models": {} } }).to_string()
    }

    #[test]
    fn the_payload_is_found_after_the_marker() {
        let stdout = format!("\n{DUMP_MARKER}\n{}\n", dump_line());
        assert_eq!(parse_dump(&stdout, &package()).unwrap().models.len(), 0);
    }

    #[test]
    fn output_printed_before_the_dump_is_ignored() {
        // Another crate's constructor, or the project's own static
        // initialization, can reach stdout first.
        let stdout = format!(
            "some other ctor said hello\n{DUMP_MARKER}\n{}\n",
            dump_line()
        );
        assert!(parse_dump(&stdout, &package()).is_ok());
    }

    #[test]
    fn a_missing_marker_points_at_the_feature_that_produces_it() {
        let err = parse_dump("hello\n", &package()).unwrap_err().to_string();
        assert!(err.contains("no schema dump"), "{err}");
        assert!(err.contains("schema-dump"), "{err}");
    }

    #[test]
    fn an_undecodable_payload_suggests_a_version_mismatch() {
        let stdout = format!("{DUMP_MARKER}\n{}\n", json!({ "unexpected": true }));
        let err = format!("{:#}", parse_dump(&stdout, &package()).unwrap_err());
        assert!(err.contains("different versions"), "{err}");
    }
}
