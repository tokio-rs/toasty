//! Schema migration types: history files, snapshots, and supporting
//! configuration.
//!
//! The migration system tracks a sequence of generated SQL migrations on disk
//! alongside the schema snapshots they were derived from. [`History`] is the
//! in-memory representation of the TOML history file; each [`HistoryEntry`]
//! records one generated migration.

mod embed;
mod generate;
mod snapshot;

pub use embed::{MigrationFile, MigrationReport, MigrationSet};
pub use generate::{Generated, generate};
pub use snapshot::Snapshot;
pub use toasty_core::migration::{History, HistoryEntry};
