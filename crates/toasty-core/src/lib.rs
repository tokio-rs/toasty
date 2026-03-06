#[macro_use]
mod macros;

pub mod driver;
pub use driver::Connection;

mod error;
pub use error::Error;

pub mod schema;
pub use schema::Schema;

pub mod stmt;

/// A Result type alias that uses Toasty's [`Error`] type.
pub type Result<T, E = Error> = core::result::Result<T, E>;

pub use async_trait::async_trait;
