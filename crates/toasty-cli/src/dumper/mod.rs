//! Extract a user's resolved schema by synthesizing and running an ephemeral
//! "dumper" crate.
//!
//! The dumper crate depends on the user's package by path, so the user's
//! `#[derive(Model)]` types end up in the dumper binary's `inventory`. The
//! binary collects every registered model, builds an [`app::Schema`] via
//! [`toasty::schema::from_macro`], and writes it as JSON to stdout. The CLI
//! parses that JSON and feeds it into [`Db::builder().app_schema(...)`] —
//! the resulting `Db` then provides both the live driver (for `migration
//! apply`) and the SQL flavor (for `migration generate`).

mod metadata;
mod run;
mod synth;

use anyhow::Result;
use std::path::Path;
use toasty_core::schema::app;

/// Extract the user's [`app::Schema`] by synthesizing, building, and running
/// a dumper crate rooted at `project_root`.
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
