#![warn(missing_docs)]
//! Toasty is an async ORM for Rust supporting both SQL (SQLite, PostgreSQL,
//! MySQL) and NoSQL (DynamoDB) databases.
//!
//! This crate is the user-facing API. It contains the database handle,
//! query execution traits, and the types that generated code builds on. For
//! a tutorial-style introduction, see the [Toasty guide].
//!
#![doc = include_str!(concat!(env!("OUT_DIR"), "/guide_link.md"))]
//!
//! # Modules
//!
//! ## [`db`] — database handle and connection pool
//!
//! The [`Db`] type is the entry point for all database interaction. It owns
//! a connection pool and provides [`Db::builder`] for configuration. The
//! module also contains [`Builder`](db::Builder) and the pool internals.
//!
//! ## [`schema`] — model, relation, and schema inspection
//!
//! The [`Model`](schema::Model) trait represents a root model that maps to a
//! database table. It is implemented by `#[derive(Model)]` — users do not
//! implement it manually. The module also contains [`Field`](schema::Field),
//! which provides schema registration and runtime helpers for field types, and
//! [`Auto`](schema::Auto), a wrapper for auto-generated values such as
//! database-assigned IDs.
//!
//! The module also provides the types that represent associations between
//! models: [`HasMany`](schema::HasMany), [`HasOne`](schema::HasOne), and
//! [`BelongsTo`](schema::BelongsTo). These appear as fields on model structs
//! and are populated through the generated relation accessors.
//!
//! The module also re-exports from `toasty-core` for inspecting the
//! app-level and db-level schema representations at runtime.
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
//! # Key traits
//!
//! - [`Model`](schema::Model) — a root model backed by a database table.
//!   Implemented by `#[derive(Model)]`.
//! - [`Embed`](schema::Embed) — an embedded type whose fields are flattened
//!   into the parent model's table. Implemented by `#[derive(Embed)]`.
//! - [`Executor`] — the dyn-compatible interface for running statements.
//!   [`Db`] and [`Transaction`] both implement it. The generic
//!   [`exec`](Executor::exec) method on `dyn Executor` accepts any typed
//!   [`Statement<T>`].
//! - [`Load`](schema::Load) — deserializes a model instance from the database
//!   value representation.
//!
//! # Other key types
//!
//! - [`Transaction`] / [`TransactionBuilder`] — transactions with
//!   auto-rollback on drop and nested savepoint support.
//! - [`Page`](stmt::Page) — a page of results from a paginated query, with cursor-based
//!   navigation.
//! - [`Batch`](stmt::Batch) — groups multiple queries into a single round-trip.
//! - [`Error`] / [`Result`] — re-exported from `toasty-core`.
//!
//! # Derive macros
//!
//! - [`#[derive(Model)]`](derive@Model) — generates the
//!   [`Model`](schema::Model) impl, query builders, create/update builders,
//!   relation accessors, and schema registration for a struct.
//! - [`#[derive(Embed)]`](derive@Embed) — generates the
//!   [`Embed`](schema::Embed) impl for a type whose fields are stored inline
//!   in a parent model's table.
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
//! - **`toasty-macros`** — the proc-macro entry points and the code generation
//!   logic they call.
//! - **`toasty-sql`** — serializes the statement AST to SQL strings. Used by
//!   the SQL driver crates.
//! - **`toasty-driver-*`** — one crate per database backend.

mod update_target;
pub use update_target::UpdateTarget;

// `Batch`, `batch()`, and `CreateMany` live in `stmt`.
pub use stmt::{batch, Batch};

/// Database handle, connection pool, executor trait, and transaction support.
pub mod db;
pub use db::{Connection, Db, Executor, Transaction, TransactionBuilder};

mod engine;

/// Model, relation, and schema inspection types.
pub mod schema;
pub use schema::{BelongsTo, HasMany, HasOne};

// `Page` lives in `stmt`.

/// Typed statement, expression, and query builder types.
pub mod stmt;
pub use stmt::Statement;

pub use toasty_macros::{create, query, Embed, Model};

pub use toasty_core::{Error, Result};

#[doc(hidden)]
pub mod codegen_support {
    pub use crate::{
        schema::{
            generate_unique_id, Auto, BelongsTo, Embed, Field, HasMany, HasOne, Load, Model,
            Register, Relation, Scope,
        },
        stmt::CreateMany,
        stmt::{self, IntoAssignment, IntoExpr, IntoInsert, IntoStatement, List, Path},
        update_target::UpdateTarget,
        Db, Error, Executor, Result, Statement,
    };
    #[cfg(feature = "serde")]
    pub use serde_json;
    pub use std::{convert::Into, default::Default, option::Option};

    pub use toasty_core as core;
}
