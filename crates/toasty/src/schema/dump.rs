//! Schema extraction for the standalone `toasty` CLI.
//!
//! `toasty migrate generate` needs the user's resolved schema, and the only
//! thing that can resolve it is a compiled artifact that links the user's
//! models: `#[derive(Model)]` registers each model through `inventory`, and
//! relations, embedded types, and `via` paths are linked across those
//! registrations at runtime. Rather than ask users to write a dumper binary,
//! `toasty` contributes one to whatever they already build.
//!
//! The [constructor](ctor) below runs before `main` in an executable, and
//! during `dlopen` in a `cdylib`. When [`DUMP_ENV`] is set it writes the
//! schema to stdout and exits; when it is not, it returns immediately and the
//! program runs normally. The CLI builds the user's own bin (or their lib as a
//! `cdylib`) and runs it with that variable set.
//!
//! The constructor is compiled only under `cfg(debug_assertions)`, so release
//! builds carry no schema-dump machinery, and only with the `schema-dump`
//! feature, which is enabled by default.

use crate::{Result, schema::DiscoverItem};

use toasty_core::schema::app::{self, ModelSet};

use std::io::{self, Write};

/// Setting this environment variable makes a Toasty-linked program dump its
/// schema and exit instead of running.
///
/// The CLI sets it only on the child process it spawns; it is never exported
/// into the user's shell. Setting it by hand is the supported way to check
/// what the CLI will see:
///
/// ```text
/// TOASTY_DUMP_SCHEMA=1 cargo run
/// ```
pub const DUMP_ENV: &str = "TOASTY_DUMP_SCHEMA";

/// Written on its own line immediately before the JSON payload.
///
/// Constructors from other crates may run — and print — before this one, and
/// nothing stops the user's own build from writing to stdout during static
/// initialization. The marker lets the CLI find the payload rather than
/// assuming it owns the stream.
pub const DUMP_MARKER: &str = "--- toasty schema dump v1 ---";

/// The envelope written to stdout, so a CLI built against a different `toasty`
/// can say so instead of failing to parse.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Dump {
    /// The version of `toasty` that produced this dump.
    pub toasty_version: String,

    /// The resolved application schema.
    pub schema: app::Schema,
}

/// Builds the application schema from every model linked into this program.
///
/// [`models!`](crate::models!) filters registrations by crate because an
/// application chooses which models a given `Db` serves. Extraction has no
/// such choice to make: every model reachable from the built artifact is part
/// of the schema the migration is generated against.
///
/// # Errors
///
/// Returns an error if the registered models do not form a valid schema — an
/// unregistered relation target, an eager-load cycle, an unresolvable `via`
/// path.
pub fn schema_from_linked_models() -> Result<app::Schema> {
    let mut models = ModelSet::new();

    for item in crate::schema::inventory::iter::<DiscoverItem> {
        item.add_to(&mut models);
    }

    app::Schema::from_macro(models)
}

/// Writes this program's schema to `out` as a marker line followed by one line
/// of JSON.
///
/// # Errors
///
/// Returns an error if the schema cannot be built or `out` cannot be written.
pub fn write_dump(out: &mut impl Write) -> Result<()> {
    let dump = Dump {
        toasty_version: env!("CARGO_PKG_VERSION").to_string(),
        schema: schema_from_linked_models()?,
    };

    let json = serde_json::to_string(&dump)
        .map_err(|err| toasty_core::Error::serialization_failure(err.to_string()))?;

    writeln!(out, "\n{DUMP_MARKER}\n{json}")
        .map_err(toasty_core::Error::driver_operation_failed)?;
    out.flush()
        .map_err(toasty_core::Error::driver_operation_failed)?;

    Ok(())
}

/// Dumps the schema to stdout and terminates the process.
///
/// Exits `0` on success and `1` after reporting a failure on stderr. Called
/// from the constructor; exposed so the path can be exercised directly.
pub fn dump_schema_to_stdout_and_exit() -> ! {
    let stdout = io::stdout();
    match write_dump(&mut stdout.lock()) {
        Ok(()) => std::process::exit(0),
        Err(err) => {
            eprintln!("toasty: failed to dump schema: {err}");
            std::process::exit(1);
        }
    }
}

/// Dumps the schema and exits when [`DUMP_ENV`] is set; otherwise returns and
/// lets the program run.
///
/// Runs before `main` for an executable and during `dlopen` for a `cdylib`.
/// Doing the work in a constructor is what makes the user's existing build
/// artifact serve as the dumper: no call site to add, and it works for a
/// library that has no `main` at all.
#[cfg(debug_assertions)]
// `linktime::ctor` places the function pointer in a platform-specific
// initializer section, which requires `unsafe`. The workspace denies
// `unsafe_code`, so the acknowledgment is scoped to this one item.
#[allow(unsafe_code)]
#[linktime::ctor(unsafe)]
fn maybe_dump_schema() {
    if std::env::var_os(DUMP_ENV).is_none() {
        return;
    }

    dump_schema_to_stdout_and_exit();
}
