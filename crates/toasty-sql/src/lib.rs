#![warn(missing_docs)]

//! SQL serialization for Toasty.
//!
//! This crate converts Toasty's statement AST into SQL strings for SQLite,
//! PostgreSQL, and MySQL. It also generates DDL statements for schema
//! migrations.

/// Schema migration statement generation.
pub mod migration;
pub use migration::*;

/// The SQL dialect being targeted.
///
/// Re-exported from `toasty-core` so callers that only need to name a dialect
/// — `toasty migrate generate --flavor postgresql` — can do so without
/// depending on the driver interface.
pub use toasty_core::driver::SqlFlavor as Flavor;

/// SQL serialization and parameter handling.
pub mod serializer;
pub use serializer::Serializer;

/// SQL statement types for both DML and DDL operations.
pub mod stmt;
pub use stmt::Statement;

/// JSON encoding for `stmt::Value`s stored in document-backed columns.
pub mod json;
