#[macro_use]
mod macros;
#[macro_use]
mod util;

mod exec_log;
pub use exec_log::ExecLog;

mod helpers;
pub use helpers::{column, columns, table_id};

mod isolate;
use isolate::Isolate;

mod logging_driver;
pub use logging_driver::{DriverOp, LoggingDriver};

mod setup;
pub use setup::Setup;

mod test;
pub use test::{Test, TestResult};

pub mod scenarios;

pub mod stmt;

/// Test implementations
pub mod tests;

// Re-export the macros
#[doc(hidden)]
pub use toasty_driver_integration_suite_macros::generate_driver_test_variants;

// Generate the test registry macro by scanning the test directory once at compile time
// This creates a macro_rules! generate_driver_tests that can be called multiple times
toasty_driver_integration_suite_macros::generate_test_registry!("src/tests");

mod prelude {
    pub(crate) use crate::{
        Test, columns,
        stmt::{Any, ArgOr},
        table_id,
    };
    pub(crate) use toasty::Result;
    pub(crate) use toasty::models;
    pub(crate) use toasty::schema::Register;
    pub(crate) type BoxError = Box<dyn std::error::Error + 'static>;

    pub(crate) use crate::util::{NumUtil, SliceUtil};
    pub(crate) use assert_struct::assert_struct;
    pub(crate) use toasty_driver_integration_suite_macros::{driver_test, scenario};
}
