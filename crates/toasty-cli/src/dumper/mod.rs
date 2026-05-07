//! Extract the user's resolved schema by synthesizing and running an
//! ephemeral "dumper" crate that path-depends on the user's package. The
//! `#[derive(Model)]` registrations land in the dumper binary's `inventory`,
//! which it collects, builds an [`app::Schema`] from, and writes as JSON to
//! stdout for the CLI to parse.

mod metadata;
mod run;
mod synth;

use anyhow::Result;
use std::path::Path;
use toasty_core::schema::app;

/// Extract the user's [`app::Schema`] from `project_root`.
pub fn extract_schema(project_root: &Path) -> Result<app::Schema> {
    let meta = metadata::load(project_root)?;
    if !meta.package.has_lib {
        anyhow::bail!(
            "package `{}` has no library target — bin-only crates are not yet supported",
            meta.package.name
        );
    }

    let synth = synth::write(&meta)?;
    run::build_and_run(&synth)
}
