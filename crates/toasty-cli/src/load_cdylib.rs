use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use toasty::schema_dump::{DUMP_SCHEMA_ENV, DUMP_TABLE_NAME_PREFIX_ENV};

/// Hidden subcommand: loads a cdylib so the schema-dump constructor inside
/// `toasty` runs. On success the constructor dumps and exits this process
/// during `Library::new`; the parent captures stdout as it would for a bin.
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
        // Set here rather than by the parent so a debug build of the CLI,
        // which links `toasty` and carries the constructor itself, does not
        // dump its own empty schema at startup.
        //
        // SAFETY: `main` runs this before building the async runtime, so the
        // process is still single-threaded.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(DUMP_SCHEMA_ENV, &self.flavor);

            // Don't let an inherited prefix through; the argument is the only
            // source for it.
            match &self.table_name_prefix {
                Some(prefix) => std::env::set_var(DUMP_TABLE_NAME_PREFIX_ENV, prefix),
                None => std::env::remove_var(DUMP_TABLE_NAME_PREFIX_ENV),
            }
        }

        // SAFETY: loading runs the library's constructors, which is the point:
        // the dump constructor prints the schema and exits. The library was
        // just built by the parent from the user's own package.
        #[allow(unsafe_code)]
        let library = unsafe { libloading::Library::new(&self.path) };

        library.with_context(|| format!("failed to load `{}`", self.path.display()))?;

        // No constructor fired (the library does not link `toasty`). Exit
        // cleanly; the parent diagnoses the missing dump.
        Ok(())
    }
}
