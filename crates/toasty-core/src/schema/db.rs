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
//! Schema diffing types live in [`super::diff`].

mod column;
pub use column::{Column, ColumnId};

mod index;
pub use index::{Index, IndexColumn, IndexId, IndexOp, IndexScope};

mod migration;
pub use migration::{AppliedMigration, Migration};

mod pk;
pub use pk::PrimaryKey;

mod schema;
pub use schema::Schema;

mod table;
pub use table::{Table, TableId};

mod ty;
mod ty_enum;
pub use ty_enum::{EnumVariant, TypeEnum};

pub use ty::Type;
