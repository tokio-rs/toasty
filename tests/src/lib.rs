#[macro_use]
mod macros;

pub mod db;

mod db_test;
pub use db_test::DbTest;

mod exec_log;
pub use exec_log::ExecLog;

pub mod helpers;
mod isolation;

mod logging_driver;
pub use logging_driver::{DriverOp, LoggingConnection, LoggingDriver};

pub mod prelude;

mod setup;
pub use setup::Setup;

pub use std_util::*;

pub mod stmt;

/// Compile-checks code snippets in guide documentation via rustdoc doctests.
/// Run with `cargo test -p tests --doc`.
#[cfg(doctest)]
mod doctests {
    #[doc = include_str!("../../docs/guide/jiff.md")]
    mod jiff {}

    #[doc = include_str!("../../docs/guide/default-and-update.md")]
    mod default_and_update {}

    #[doc = include_str!("../../docs/guide/transactions.md")]
    mod transactions {}

    #[doc = include_str!("../../docs/guide/pagination.md")]
    mod pagination {}
}
