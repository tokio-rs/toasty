//! Core types and abstractions for the Toasty ORM.
//!
//! This crate provides the shared foundation used by all Toasty components:
//!
//! - [`Schema`] -- the combined app-level, database-level, and mapping schema
//! - [`stmt`] -- the statement AST used to represent queries and mutations
//! - [`driver`] -- the trait interface that database drivers implement
//! - [`Error`] / [`Result`] -- unified error handling
//!
//! Most users interact with the higher-level `toasty` crate. This crate is
//! relevant when writing database drivers or working with schema internals.
//!
//! # Examples
//!
//! ```ignore
//! use toasty_core::{Schema, Error, Result};
//!
//! fn check_schema(schema: &Schema) -> Result<()> {
//!     println!("models: {}", schema.app.models.len());
//!     Ok(())
//! }
//! ```

#[macro_use]
mod macros;

/// Database driver traits and capability descriptions.
pub mod driver;
pub use driver::Connection;

mod error;
/// The error type returned by Toasty operations.
pub use error::Error;

/// Schema definitions spanning the app layer, database layer, and the mapping
/// between them.
pub mod schema;
pub use schema::Schema;

/// Statement AST types for representing queries, inserts, updates, and deletes.
pub mod stmt;

/// A `Result` type alias that uses Toasty's [`Error`] type.
///
/// This is the standard return type for fallible operations throughout
/// `toasty-core`.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::Result;
///
/// fn validate_name(name: &str) -> Result<()> {
///     if name.is_empty() {
///         return Err(toasty_core::Error::invalid_schema("name must not be empty"));
///     }
///     Ok(())
/// }
/// ```
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Re-export of `async_trait` for use by driver implementations.
pub use async_trait::async_trait;
