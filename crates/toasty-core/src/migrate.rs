//! Migration types for Toasty schema management.
//!
//! This module contains the configuration, history file, and related types
//! used by the migration system. Most types are available unconditionally,
//! but file I/O operations (load/save) require the `migrate` feature.

mod config;
pub use config::{MigrationConfig, MigrationPrefixStyle};

mod history_file;
pub use history_file::{HistoryFile, HistoryFileMigration};
