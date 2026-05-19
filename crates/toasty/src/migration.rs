//! Schema migration types: history files, snapshots, and supporting
//! configuration.
//!
//! The migration system tracks a sequence of generated SQL migrations on disk
//! alongside the schema snapshots they were derived from. [`History`] is the
//! in-memory representation of the TOML history file; each [`HistoryEntry`]
//! records one generated migration.

mod history;
pub use history::{History, HistoryEntry};
