#[macro_use]
mod macros;

mod exec_log;
use exec_log::ExecLog;

mod isolate;
use isolate::Isolate;

mod logging_driver;
use logging_driver::LoggingDriver;

mod setup;
pub use setup::Setup;

mod test;
pub use test::Test;

/// Test implementations
pub mod tests;

// Re-export the macros
#[doc(hidden)]
pub use toasty_driver_integration_suite_macros::generate_driver_test_variants;

// Generate the test registry macro by scanning the test directory once at compile time
// This creates a macro_rules! generate_driver_tests that can be called multiple times
toasty_driver_integration_suite_macros::generate_test_registry!("src/tests");

mod prelude {
    pub(crate) use crate::Test;

    pub(crate) use std_util::{
        assert_err, assert_none, assert_ok, assert_unique, num::NumUtil, slice::SliceUtil,
    };
    pub(crate) use toasty_driver_integration_suite_macros::driver_test;
}
