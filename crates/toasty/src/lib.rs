//! The main Toasty crate. This is the user-facing API for defining models,
//! querying the database, and managing connections.
//!
//! For a guided introduction, see the [Toasty guide]. These API docs serve as a
//! reference for the types and traits in this crate.
//!
//! [Toasty guide]: https://toasty-rs.github.io/toasty/guide/
//!
//! # Defining models
//!
//! Models are Rust structs annotated with `#[derive(Model)]`. The derive macro
//! generates query builders, create/update builders, relation accessors, and
//! the runtime schema registration needed to interact with the database.
//!
//! ```ignore
//! #[derive(Model)]
//! struct User {
//!     #[key]
//!     #[auto]
//!     id: Id<User>,
//!
//!     name: String,
//!
//!     #[has_many]
//!     posts: HasMany<Post>,
//! }
//! ```
//!
//! The [`Model`] trait is implemented by root models that map to database
//! tables. The [`Embed`] trait is for embedded types whose fields are flattened
//! into the parent model's table — they have no table or primary key of their
//! own.
//!
//! # Connecting to a database
//!
//! Use [`Db`] to open a connection. Toasty ships optional, feature-gated
//! driver crates for each supported database:
//!
//! | Feature        | Driver crate                   |
//! |----------------|--------------------------------|
//! | `sqlite`       | `toasty-driver-sqlite`         |
//! | `postgresql`   | `toasty-driver-postgresql`     |
//! | `mysql`        | `toasty-driver-mysql`          |
//! | `dynamodb`     | `toasty-driver-dynamodb`       |
//!
//! ```ignore
//! let db = Db::builder()
//!     .connect("sqlite::memory:")
//!     .await?;
//! ```
//!
//! The [`db`] module contains the connection [`Builder`](db::Builder), the
//! connection pool, and the `connect` helpers.
//!
//! # Executing queries
//!
//! Both [`Db`] and [`Transaction`] implement the [`Executor`] trait, which is
//! the low-level, dyn-compatible interface for running statements. Generic
//! convenience methods — [`all`](ExecutorExt::all),
//! [`first`](ExecutorExt::first), [`get`](ExecutorExt::get),
//! [`delete`](ExecutorExt::delete) — live on [`ExecutorExt`], which is
//! blanket-implemented for every `Executor`.
//!
//! In practice, most queries go through the generated query builders rather
//! than calling `ExecutorExt` methods directly:
//!
//! ```ignore
//! // Generated `find_by_name` returns a query builder
//! let user = User::find_by_name("Alice").get(&mut db).await?;
//! ```
//!
//! # Transactions
//!
//! Start a transaction with [`Executor::transaction`] or configure one with
//! [`Db::transaction_builder`](Db::transaction_builder). Transactions
//! auto-rollback on drop if neither
//! [`commit`](Transaction::commit) nor [`rollback`](Transaction::rollback) is
//! called. Nested transactions use savepoints.
//!
//! # Module overview
//!
//! | Module       | Contents |
//! |--------------|----------|
//! | [`db`]       | [`Db`] handle, connection [`Builder`](db::Builder), and pool |
//! | [`model`]    | [`Model`] trait, [`Field`](model::Field) and [`Auto`](model::Auto) helpers |
//! | [`stmt`]     | Typed statement types — [`Query`](stmt::Query), [`Insert`](stmt::Insert), [`Update`](stmt::Update), [`Delete`](stmt::Delete), expression helpers |
//! | [`relation`] | Relation field types — [`HasMany`], [`HasOne`], [`BelongsTo`] |
//! | [`schema`]   | Re-exports from `toasty-core` for schema inspection |
//! | [`driver`]   | Re-exports from `toasty-core` for the database driver interface |
//!
//! # Crate boundaries
//!
//! `toasty` is the user-facing crate. It depends on several internal crates
//! that are not meant to be used directly:
//!
//! - **`toasty-core`** — shared types: schema representations, the statement
//!   AST, the [`Driver`](driver::Driver) trait, [`Error`], and [`Result`].
//! - **`toasty-macros`** / **`toasty-codegen`** — the `#[derive(Model)]` and
//!   `#[derive(Embed)]` proc-macros and the code they generate.
//! - **`toasty-sql`** — SQL serialization (statement AST to SQL string), used
//!   by the SQL driver crates.
//! - **`toasty-driver-*`** — database driver implementations, one per backend.

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
