//! service-ops (migrate binary): a project-local migration CLI built from `toasty-cli`. The
//! tool is YOUR binary — not a global command — because diffing the schema needs your compiled
//! model types. Run it from this crate's directory (so it finds `Toasty.toml`):
//!
//!   cargo run -p example-service-ops --bin migrate -- migration generate --name initial
//!   cargo run -p example-service-ops --bin migrate -- migration apply
//!
//! `generate` diffs the models against the stored snapshot and writes incremental SQL under
//! `toasty/`; `apply` runs the pending files and records them in a `__toasty_migrations` table.

use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Reads Toasty.toml ([migration] path = "toasty").
    let config = Config::load()?;

    // Same models and connection as the server, via the shared library.
    let db = example_service_ops::build_db().await?;

    // ToastyCli exposes the `migration generate/apply/...` subcommands over your models.
    ToastyCli::with_config(db, config).parse_and_run().await?;

    Ok(())
}
