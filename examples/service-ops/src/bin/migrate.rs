//! service-ops (migrate binary): the migration commands embedded in a binary of this project's
//! own, built from `toasty-cli` as a library. Run it from this crate's directory (so it finds
//! `Toasty.toml`):
//!
//!   cargo run -p example-service-ops --bin migrate -- migration generate --name initial
//!   cargo run -p example-service-ops --bin migrate -- migration apply
//!
//! `generate` diffs the models against the stored snapshot and writes incremental SQL under
//! `toasty/`; `apply` runs the pending files and records them in a `__toasty_migrations` table.
//!
//! Embedding is optional. `cargo install toasty-cli` gives you a `toasty` command that does the
//! same thing for any project without a binary like this one — it builds the package and reads
//! the schema out of the artifact. Ship a binary like this when a deployment should carry its
//! own migration command rather than depend on an installed tool. Because it links the models
//! and connects to a database, it needs neither `--flavor` nor `--url`.

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
