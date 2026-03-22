//! Database-level schema definitions.
//!
//! This module represents what the database sees: tables, columns, indices,
//! primary keys, storage types, and migrations. It is the counterpart to the
//! application-level schema in [`super::app`], with the two layers connected
//! by the [`super::mapping`] module.
//!
//! # Key types
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`Schema`] | Collection of all tables in a database |
//! | [`Table`] | A single database table with columns, indices, and a primary key |
//! | [`Column`] | A column within a table, including its name, type, and constraints |
//! | [`Index`] | A database index over one or more columns |
//! | [`Type`] | Database storage type (e.g. `Integer(4)`, `Text`, `VarChar(255)`) |
//! | [`PrimaryKey`] | The primary key definition for a table |
//! | [`Migration`] | A SQL migration generated from a schema diff |
//!
//! # Schema diffing
//!
//! The module provides diff types ([`SchemaDiff`], [`TablesDiff`], [`ColumnsDiff`],
//! [`IndicesDiff`]) that compare two schema versions and produce a list of
//! structural changes. [`RenameHints`] lets callers indicate which items were
//! renamed (rather than dropped and recreated).

mod column;
pub use column::{Column, ColumnId, ColumnsDiff, ColumnsDiffItem};

mod diff;
pub use diff::{DiffContext, RenameHints};

mod index;
pub use index::{Index, IndexColumn, IndexId, IndexOp, IndexScope, IndicesDiff, IndicesDiffItem};

mod migration;
pub use migration::{AppliedMigration, Migration};

mod pk;
pub use pk::PrimaryKey;

mod schema;
pub use schema::{Schema, SchemaDiff};

mod table;
pub use table::{Table, TableId, TablesDiff, TablesDiffItem};

mod ty;
pub use ty::Type;
