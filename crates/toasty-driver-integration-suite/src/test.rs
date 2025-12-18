use std::sync::{Arc, Mutex};

use toasty::{driver::Driver, Db};
use tokio::runtime::Runtime;

use crate::{exec_log::ExecLog, logging_driver::LoggingDriver};

/// Wraps the Tokio runtime and ensures cleanup happens.
///
/// This also passes necessary
pub(crate) struct Test {
    driver: Arc<dyn Driver>,
    runtime: Option<Runtime>,
    exec_log: ExecLog,
}

impl Test {
    pub(crate) fn new(driver: Arc<dyn Driver>) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create Tokio runtime");

        Test {
            driver,
            runtime: Some(runtime),
            exec_log: ExecLog::new(Arc::new(Mutex::new(Vec::new()))),
        }
    }

    /// Try to setup a database with models, returns Result for error handling
    pub async fn try_setup_db(&mut self, mut builder: toasty::db::Builder) -> toasty::Result<Db> {
        /*
        let setup = self.setup.as_ref().expect("Setup already consumed");

        // Let the setup configure the builder
        setup.configure_builder(&mut builder);
        */

        // Always wrap with logging
        let logging_driver = LoggingDriver::new(self.driver.clone());
        let ops_log = logging_driver.ops_log_handle();
        self.exec_log = ExecLog::new(ops_log);

        // Build the database with the logging driver
        let db = builder.build(logging_driver).await?;
        db.reset_db().await?;

        Ok(db)
    }

    /// Setup a database with models, always with logging enabled
    pub(crate) async fn setup_db(&mut self, builder: toasty::db::Builder) -> Db {
        self.try_setup_db(builder).await.unwrap()
    }

    /// Run an async test function using the internal runtime
    pub(crate) fn run<F>(&mut self, f: F)
    where
        F: for<'a> FnOnce(
            &'a mut Self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>>,
    {
        // Temporarily take the runtime to avoid borrow checker issues
        let runtime = self.runtime.take().expect("runtime already consumed");
        let fut = f(self);
        runtime.block_on(fut);
        self.runtime = Some(runtime);
    }
}
