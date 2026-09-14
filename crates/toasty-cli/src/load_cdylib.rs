use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use toasty::schema_dump::{DUMP_SCHEMA_ENV, DUMP_TABLE_NAME_PREFIX_ENV};

/// Hidden subcommand: loads a cdylib so the schema-dump constructor inside
/// `toasty` runs.
///
/// The constructor dumps the schema to stdout and exits this process during
/// `Library::new`, so on success the code after the load is never reached.
/// The parent `toasty` process captures stdout exactly as it would for a bin
/// artifact.
#[derive(Parser, Debug)]
pub struct LoadCdylibCommand {
    /// Path of the cdylib to load
    path: PathBuf,

    /// Flavor to dump the schema for
    #[arg(long)]
    flavor: String,

    /// Table name prefix to lower the schema with
    #[arg(long)]
    table_name_prefix: Option<String>,
}

impl LoadCdylibCommand {
    pub(crate) fn run(self) -> Result<()> {
        // The variable is set here rather than by the parent so that a debug
        // build of the CLI (which links `toasty` and therefore carries the
        // dump constructor itself) does not dump its own — empty — schema at
        // startup before reaching this subcommand.
        //
        // SAFETY: the process is single-threaded at this point. `main` runs
        // this subcommand before building the async runtime precisely so that
        // no other thread can be reading the environment concurrently.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(DUMP_SCHEMA_ENV, &self.flavor);

            // This process inherited the parent's environment, so an ambient
            // prefix would silently rename every table. `Toasty.toml`, passed
            // through as an argument, is the only source for it.
            match &self.table_name_prefix {
                Some(prefix) => std::env::set_var(DUMP_TABLE_NAME_PREFIX_ENV, prefix),
                None => std::env::remove_var(DUMP_TABLE_NAME_PREFIX_ENV),
            }
        }

        // SAFETY: loading a library runs its initialization code, which for a
        // Toasty cdylib is exactly the point: the dump constructor prints the
        // schema and exits the process. The library is a dev-profile artifact
        // the parent process just built from the user's own package.
        #[allow(unsafe_code)]
        let library = unsafe { libloading::Library::new(&self.path) };

        library.with_context(|| format!("failed to load `{}`", self.path.display()))?;

        // Reached only if the constructor did not fire — e.g. the library does
        // not actually link `toasty`. Exit cleanly with no dump on stdout: the
        // parent knows which package it selected and how `toasty` is reached
        // from it, so it words the diagnosis far better than this can.
        Ok(())
    }
}
