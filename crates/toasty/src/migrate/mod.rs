//! Programmatic database migration management.
//!
//! `toasty::migrate` exposes the building blocks behind the `toasty-cli`
//! migration subcommands so that applications can apply, generate, drop, and
//! inspect migrations without spawning a CLI. Migration metadata is stored
//! alongside SQL files in a directory tree configured by [`Config`].
//!
//! The most common use case is applying pending migrations on application
//! startup:
//!
//! ```no_run
//! # async fn run() -> toasty::Result<()> {
//! let db = toasty::Db::builder().connect("sqlite::memory:").await?;
//! let config = toasty::migrate::Config::default();
//!
//! toasty::migrate::apply(&db, &config).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Layout on disk
//!
//! Given a [`Config`] with `path = "db"`, files are organized as:
//!
//! ```text
//! db/
//!   history.toml             # records every generated migration
//!   migrations/
//!     0001_init.sql          # generated SQL applied to the database
//!   snapshots/
//!     0001_snapshot.toml     # schema captured at the time of generation
//! ```
//!
//! [`HistoryFile`] is the source of truth for what migrations exist;
//! [`SnapshotFile`] records the schema at the point each migration was
//! generated and is consumed by the next [`generate`] call to compute a diff.

mod apply;
mod config;
mod drop;
mod generate;
mod history_file;
mod snapshot_file;

pub use apply::{apply, pending};
pub use config::{Config, PrefixStyle};
pub use drop::{DropTarget, Dropped, drop_migration};
pub use generate::{Generated, generate};
pub use history_file::{HistoryFile, HistoryFileMigration};
pub use snapshot_file::SnapshotFile;

use crate::Result;
use toasty_core::Error;
use toasty_core::schema::db::Schema;

/// Loads the database schema captured by the most recent snapshot, or returns
/// an empty schema when no migrations have been generated yet.
///
/// This is the same starting point [`generate`] uses when computing a schema
/// diff. Callers that want to drive interactive rename detection (as the
/// `toasty-cli` `generate` command does) can load both the previous and
/// current schemas and compute their own [`SchemaDiff`] before invoking
/// [`generate`].
///
/// [`SchemaDiff`]: toasty_core::schema::db::SchemaDiff
pub fn previous_schema(config: &Config) -> Result<Schema> {
    let history = HistoryFile::load_or_default(config.history_file_path())?;

    let Some(latest) = history.migrations().last() else {
        return Ok(Schema::default());
    };

    let snapshot_path = config.snapshots_dir().join(&latest.snapshot_name);
    let snapshot = SnapshotFile::load(&snapshot_path)?;
    Ok(snapshot.schema)
}

/// Wraps an arbitrary error as a Toasty [`Error`] using the provided context.
pub(crate) fn err_ctx(ctx: impl std::fmt::Display, source: impl std::fmt::Display) -> Error {
    Error::from_args(format_args!("{ctx}: {source}"))
}

/// Constructs a Toasty [`Error`] from a display message.
pub(crate) fn err(msg: impl std::fmt::Display) -> Error {
    Error::from_args(format_args!("{msg}"))
}
