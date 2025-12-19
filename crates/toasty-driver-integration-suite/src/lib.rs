#[macro_use]
mod macros;

mod cx;

mod exec_log;
use exec_log::ExecLog;

mod isolate;
use isolate::Isolate;

mod logging_driver;
use logging_driver::LoggingDriver;

pub mod registry;

mod setup;
pub use setup::Setup;

mod suite;
pub use suite::IntegrationSuite;

mod test;
use test::Test;

/// Test implementations
pub(crate) mod tests;

mod prelude {
    pub(crate) use crate::Test;

    pub(crate) use std_util::{
        assert_err, assert_none, assert_ok, assert_unique, num::NumUtil, slice::SliceUtil,
    };
    pub(crate) use toasty_driver_integration_suite_macros::driver_test;
}
