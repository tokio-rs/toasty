use crate::{
    logging_driver::{DriverOp, LoggingDriver},
    Setup,
};
use std::sync::{Arc, Mutex};
use toasty::Db;

/// Internal wrapper that manages the Tokio runtime and ensures cleanup happens.
///
/// This is an implementation detail that allows us to:
/// 1. Use #[test] instead of #[tokio::test] for better control
/// 2. Ensure cleanup blocks before the test process exits
/// 3. Keep the existing test API unchanged
/// 4. Always log driver operations for debugging
pub struct DbTest<S: Setup> {
    runtime: tokio::runtime::Runtime,
    setup: Option<S>,
    ops_log: Arc<Mutex<Vec<DriverOp>>>,
}

impl<S: Setup> DbTest<S> {
    /// Create a new DbTest with a current-thread runtime.
    pub fn new(setup: S) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");

        Self {
            runtime,
            setup: Some(setup),
            ops_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Try to setup a database with models, returns Result for error handling
    pub async fn try_setup_db(&mut self, mut builder: toasty::db::Builder) -> toasty::Result<Db> {
        let setup = self.setup.as_ref().expect("Setup already consumed");

        // Let the setup configure the builder
        setup.configure_builder(&mut builder);

        // Get the driver from the setup
        let driver = setup.connect().await?;

        // Always wrap with logging, using our existing ops_log
        let logging_driver = LoggingDriver::new(Box::new(driver));
        self.ops_log = logging_driver.ops_log_handle();

        // Build the database with the logging driver
        let db = builder.build(logging_driver).await?;
        db.reset_db().await?;

        Ok(db)
    }

    /// Setup a database with models, always with logging enabled
    pub async fn setup_db(&mut self, builder: toasty::db::Builder) -> Db {
        self.try_setup_db(builder).await.unwrap()
    }

    /// Get the operations log for assertions
    pub fn ops_log(&self) -> Arc<Mutex<Vec<DriverOp>>> {
        self.ops_log.clone()
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

    /// Connect to get a raw driver (for error testing)
    pub async fn connect(&self) -> toasty::Result<<S as Setup>::Driver> {
        let setup = self.setup.as_ref().expect("Setup already consumed");
        setup.connect().await
    }

    /// Get raw column value from database (for storage verification)
    pub async fn get_raw_column_value<T>(
        &self,
        table: &str,
        column: &str,
        filter: std::collections::HashMap<String, toasty_core::stmt::Value>,
    ) -> toasty::Result<T>
    where
        T: TryFrom<toasty_core::stmt::Value, Error = toasty_core::Error>,
    {
        let setup = self.setup.as_ref().expect("Setup already consumed");
        let value = setup.get_raw_column_value(table, column, filter).await?;
        T::try_from(value).map_err(Into::into)
    }

    /// Run a test function with a mutable reference to self, using our managed runtime.
    pub fn run_test<F>(&mut self, test_fn: F)
    where
        F: for<'a> FnOnce(
            &'a mut DbTest<S>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>>,
    {
        // Use unsafe to get a mutable reference to self inside the closure
        // This is safe because we control the runtime and know there's no aliasing
        let self_ptr = self as *mut DbTest<S>;

        self.runtime.block_on(async {
            let self_mut = unsafe { &mut *self_ptr };
            test_fn(self_mut).await;
        });
    }
}

impl<S: Setup> Drop for DbTest<S> {
    fn drop(&mut self) {
        // If setup is still present, clean it up
        if let Some(setup) = self.setup.take() {
            self.runtime.block_on(async {
                let _ = setup.cleanup_my_tables().await;
            });
        }
    }
}
