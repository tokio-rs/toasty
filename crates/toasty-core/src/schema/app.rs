//! Application-level schema definitions for models, fields, relations, and indices.
//!
//! This module contains the types that represent a Toasty schema from the
//! application's perspective: models with named fields, relationships between
//! models, primary keys, indices, and constraints. This is the layer that Rust
//! code interacts with; the separate [`super::db`] module represents the
//! physical database schema (tables, columns), and [`super::mapping`] bridges
//! the two.
//!
//! # Key types
//!
//! - [`Schema`] -- the top-level container holding all registered models.
//! - [`Model`] -- a single model, which can be a [`ModelRoot`] (backed by a
//!   database table), an [`EmbeddedStruct`], or an [`EmbeddedEnum`].
//! - [`Field`] -- one field on a model, identified by a [`FieldId`].
//! - [`BelongsTo`], [`HasMany`], [`HasOne`] -- relation types.
//! - [`Index`] -- a secondary index on a model's fields.
//! - [`PrimaryKey`] -- a model's primary key definition.
//!
//! # Examples
//!
//! ```ignore
//! use toasty_core::schema::app::Schema;
//!
//! // Schemas are typically constructed via the derive macro or `Schema::from_macro`.
//! let schema = Schema::default();
//! assert_eq!(schema.models().count(), 0);
//! ```

mod arg;
pub use arg::Arg;

mod auto;
pub use auto::{AutoStrategy, UuidVersion};

mod constraint;
pub use constraint::{Constraint, ConstraintLength};

mod embedded;
pub use embedded::Embedded;

mod field;
pub use field::{Field, FieldId, FieldName, FieldPrimitive, FieldTy, SerializeFormat};

mod fk;
pub use fk::{ForeignKey, ForeignKeyField};

mod index;
pub use index::{Index, IndexField, IndexId};

mod model;
pub use model::{
    EmbeddedEnum, EmbeddedStruct, EnumVariant, Model, ModelId, ModelRoot, ModelSet, VariantId,
};

mod pk;
pub use pk::PrimaryKey;

mod relation;
pub use relation::{BelongsTo, HasMany, HasOne};

mod schema;
pub use schema::{Resolved, Schema};

use super::Name;
