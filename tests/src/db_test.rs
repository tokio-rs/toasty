use crate::{exec_log::ExecLog, logging_driver::LoggingDriver, Setup};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use toasty::Db;
use toasty_core::stmt;

/// Internal wrapper that manages the Tokio runtime and ensures cleanup happens.
///
/// This is an implementation detail that allows us to:
/// 1. Use #[test] instead of #[tokio::test] for better control
/// 2. Ensure cleanup blocks before the test process exits
/// 3. Keep the existing test API unchanged
/// 4. Always log driver operations for debugging
pub struct DbTest {
    runtime: tokio::runtime::Runtime,
    setup: Option<Box<dyn Setup>>,
    exec_log: ExecLog,
}

impl DbTest {
    /// Create a new DbTest with a current-thread runtime.
    pub fn new(setup: Box<dyn Setup>) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        Self {
            runtime,
            setup: Some(setup),
            exec_log: ExecLog::new(Arc::new(Mutex::new(Vec::new()))),
        }
    }

    /// Try to setup a database with models, returns Result for error handling
    pub async fn try_setup_db(&mut self, mut builder: toasty::db::Builder) -> toasty::Result<Db> {
        let setup = self.setup.as_ref().expect("Setup already consumed");

        // Let the setup configure the builder
        setup.configure_builder(&mut builder);

        // Always wrap with logging
        let logging_driver = LoggingDriver::new(setup.driver());
        let ops_log = logging_driver.ops_log_handle();
        self.exec_log = ExecLog::new(ops_log);

        // Build the database with the logging driver
        let db = builder.build(logging_driver).await?;
        db.push_schema().await?;

        Ok(db)
    }

    /// Setup a database with models, always with logging enabled
    pub async fn setup_db(&mut self, builder: toasty::db::Builder) -> Db {
        self.try_setup_db(builder).await.unwrap()
    }

    /// Get the execution log for assertions
    pub fn log(&mut self) -> &mut ExecLog {
        &mut self.exec_log
    }

    /// Get capability information from the setup
    pub fn capability(&self) -> &toasty::driver::Capability {
        self.setup
            .as_ref()
            .expect("Setup already consumed")
            .capability()
    }
    /// Configure a builder (for error testing)
    pub fn configure_builder(&self, builder: &mut toasty::db::Builder) {
        let setup = self.setup.as_ref().expect("Setup already consumed");
        setup.configure_builder(builder);
    }

    /// Get raw column value from database (for storage verification)
    pub async fn get_raw_column_value<T>(
        &self,
        table: &str,
        column: &str,
        filter: HashMap<String, stmt::Value>,
    ) -> toasty::Result<T>
    where
        T: TryFrom<stmt::Value, Error = toasty_core::Error>,
    {
        let setup = self.setup.as_ref().expect("Setup already consumed");
        let value = setup.get_raw_column_value(table, column, filter).await?;
        T::try_from(value)
    }

    /// Run a test function with a mutable reference to self, using our managed runtime.
    pub fn run_test<F>(&mut self, test_fn: F)
    where
        F: for<'a> FnOnce(
            &'a mut DbTest,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>>,
    {
        // Use unsafe to get a mutable reference to self inside the closure
        // This is safe because we control the runtime and know there's no aliasing
        let self_ptr = self as *mut DbTest;

        self.runtime.block_on(async {
            let self_mut = unsafe { &mut *self_ptr };
            test_fn(self_mut).await;
        });
    }
}

impl Drop for DbTest {
    fn drop(&mut self) {
        // If setup is still present, clean it up
        if let Some(setup) = self.setup.take() {
            self.runtime.block_on(async {
                let _ = setup.cleanup_my_tables().await;
            });
        }
    }
}
