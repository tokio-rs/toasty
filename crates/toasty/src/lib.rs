//! Toasty is an async ORM for Rust supporting both SQL (SQLite, PostgreSQL,
//! MySQL) and NoSQL (DynamoDB) databases.
//!
//! This crate is the user-facing API. It contains the database handle,
//! query execution traits, and the types that generated code builds on. For
//! a tutorial-style introduction, see the [Toasty guide].
//!
//! [Toasty guide]: ../../guide/
//!
//! # Modules
//!
//! ## [`db`] — database handle and connection pool
//!
//! The [`Db`] type is the entry point for all database interaction. It owns
//! a connection pool and provides [`Db::builder`] for configuration. The
//! module also contains [`Builder`](db::Builder) and the pool internals.
//!
//! ## [`model`] — model trait and field helpers
//!
//! The [`Model`] trait represents a root model that maps to a database table.
//! It is implemented by `#[derive(Model)]` — users do not implement it
//! manually. The module also contains [`Field`](model::Field), which
//! describes a typed field accessor, and [`Auto`](model::Auto), a wrapper
//! for auto-generated values such as database-assigned IDs.
//!
//! ## [`stmt`] — typed statement and expression types
//!
//! Contains the typed wrappers around the statement AST:
//! [`Query`](stmt::Query), [`Insert`](stmt::Insert),
//! [`Update`](stmt::Update), [`Delete`](stmt::Delete), and
//! [`Statement`]. Also includes expression helpers like [`Expr`](stmt::Expr),
//! [`Path`](stmt::Path), and the [`in_list`](stmt::in_list) function.
//! Generated query builders (e.g. `find_by_*`, `filter_by_*`) produce these
//! types.
//!
//! ## [`relation`] — relation field types
//!
//! The types that represent associations between models: [`HasMany`],
//! [`HasOne`], and [`BelongsTo`]. These appear as fields on model structs
//! and are populated through the generated relation accessors.
//!
//! ## [`schema`] — schema inspection
//!
//! Re-exports from `toasty-core` for inspecting the app-level and db-level
//! schema representations at runtime.
//!
//! ## [`driver`] — database driver interface
//!
//! Re-exports from `toasty-core`. The [`Driver`](driver::Driver) and
//! [`Connection`](driver::Connection) traits define the interface that each
//! database backend implements. Users interact with drivers indirectly
//! through [`Db`].
//!
//! # Key traits
//!
//! - [`Model`] — a root model backed by a database table. Implemented by
//!   `#[derive(Model)]`.
//! - [`Embed`] — an embedded type whose fields are flattened into the parent
//!   model's table. Implemented by `#[derive(Embed)]`.
//! - [`Executor`] — the dyn-compatible interface for running statements.
//!   [`Db`] and [`Transaction`] both implement it.
//! - [`ExecutorExt`] — generic convenience methods ([`all`](ExecutorExt::all),
//!   [`first`](ExecutorExt::first), [`get`](ExecutorExt::get),
//!   [`delete`](ExecutorExt::delete)) blanket-implemented for every
//!   `Executor`.
//! - [`Load`] — deserializes a model instance from the database value
//!   representation.
//!
//! # Other key types
//!
//! - [`Transaction`] / [`TransactionBuilder`] — transactions with
//!   auto-rollback on drop and nested savepoint support.
//! - [`Page`] — a page of results from a paginated query, with cursor-based
//!   navigation.
//! - [`Batch`] — groups multiple queries into a single round-trip.
//! - [`Error`] / [`Result`] — re-exported from `toasty-core`.
//!
//! # Derive macros
//!
//! - [`#[derive(Model)]`](derive@Model) — generates the [`Model`] impl,
//!   query builders, create/update builders, relation accessors, and schema
//!   registration for a struct.
//! - [`#[derive(Embed)]`](derive@Embed) — generates the [`Embed`] impl for a
//!   type whose fields are stored inline in a parent model's table.
//!
//! # Feature flags
//!
//! Each database driver is behind an optional feature flag:
//!
//! | Feature        | Driver crate                 |
//! |----------------|------------------------------|
//! | `sqlite`       | `toasty-driver-sqlite`       |
//! | `postgresql`   | `toasty-driver-postgresql`   |
//! | `mysql`        | `toasty-driver-mysql`        |
//! | `dynamodb`     | `toasty-driver-dynamodb`     |
//!
//! Additional feature flags: `rust_decimal`, `bigdecimal`, `jiff` (date/time
//! via the `jiff` crate), and `serde` (JSON serialization support).
//!
//! # Other crates in the workspace
//!
//! `toasty` depends on several internal crates that most users do not need
//! to use directly:
//!
//! - **`toasty-core`** — shared types used across the workspace: the schema
//!   representations (app-level and db-level), the statement AST, the
//!   [`Driver`](driver::Driver) trait, and [`Error`] / [`Result`].
//! - **`toasty-macros`** / **`toasty-codegen`** — the proc-macro entry points
//!   and the code generation logic they call.
//! - **`toasty-sql`** — serializes the statement AST to SQL strings. Used by
//!   the SQL driver crates.
//! - **`toasty-driver-*`** — one crate per database backend.

mod apply_update;
pub use apply_update::{ApplyUpdate, Query};

mod batch;
pub use batch::{batch, Batch, CreateMany};

pub mod db;
pub use db::Db;

mod embed;
pub use embed::Embed;

mod executor;
pub use executor::Executor;

mod executor_ext;
pub use executor_ext::ExecutorExt;

mod engine;

mod load;
pub use load::Load;

pub mod model;
pub use model::Model;

mod register;
pub use register::Register;

mod page;
pub use page::Page;

pub mod relation;
pub use relation::{BelongsTo, HasMany, HasOne};

pub mod schema;

pub mod stmt;
pub use stmt::Statement;

mod transaction;
pub use transaction::{Transaction, TransactionBuilder};

pub use toasty_macros::{create, query, Embed, Model};

pub use toasty_core::{Error, Result};

#[doc(hidden)]
pub mod codegen_support {
    pub use crate::{
        apply_update::{ApplyUpdate, Query},
        batch::CreateMany,
        model::{Auto, Field},
        register::generate_unique_id,
        relation::Relation,
        relation::{BelongsTo, HasMany, HasOne},
        stmt::{self, IntoExpr, IntoInsert, IntoStatement, List, Path},
        Db, Embed, Error, Executor, ExecutorExt, Load, Model, Register, Result, Statement,
    };
    #[cfg(feature = "serde")]
    pub use serde_json;
    pub use std::{convert::Into, default::Default, option::Option};

    pub use toasty_core as core;
    pub use toasty_core::{
        driver,
        schema::{
            self,
            app::{FieldId, ModelId},
        },
        stmt::{Type, Value, ValueRecord, ValueStream},
    };
}

pub mod driver {
    pub use toasty_core::driver::*;
}

pub use toasty_core::driver::IsolationLevel;
