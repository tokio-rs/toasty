use std::{
    error::Error,
    sync::{Arc, RwLock},
};

use toasty::{Db, schema::ModelSet};
use tokio::runtime::Runtime;

use crate::{Fault, InstrumentedDriver, InstrumentedHandle, Isolate, Setup};

/// Global lock for coordinating serial vs parallel tests.
/// Normal tests acquire a read lock (allowing parallelism).
/// Serial tests acquire a write lock (exclusive access).
static TEST_LOCK: RwLock<()> = RwLock::new(());

/// Wraps the Tokio runtime and ensures cleanup happens.
///
/// This also passes necessary
pub struct Test {
    /// Handle to the DB suite setup
    setup: Arc<dyn Setup>,

    /// Handles isolating tables between tests
    isolate: Isolate,

    /// Tokio runtime used by the test
    runtime: Option<Runtime>,

    /// Single handle controlling the instrumented driver test middleware:
    /// the operations log and the fault-injection queue. Populated by
    /// `try_setup_db_with`.
    handle: InstrumentedHandle,

    /// List of all tables created during the test. These will need to be removed later.
    tables: Vec<String>,

    /// Whether this test requires exclusive (serial) execution
    serial: bool,
}

impl Test {
    pub fn new(setup: Arc<dyn Setup>) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create Tokio runtime");

        Test {
            setup,
            isolate: Isolate::new(),
            runtime: Some(runtime),
            handle: InstrumentedHandle::default(),
            tables: vec![],
            serial: false,
        }
    }

    /// Try to setup a database with models, returns Result for error handling
    pub async fn try_setup_db(&mut self, models: ModelSet) -> toasty::Result<Db> {
        self.try_setup_db_with(models, |_| {}).await
    }

    /// Try to setup a database with models, allowing the caller to customize
    /// the [`toasty::db::Builder`] before it is built (e.g., to set pool
    /// configuration).
    pub async fn try_setup_db_with(
        &mut self,
        models: ModelSet,
        customize: impl FnOnce(&mut toasty::db::Builder),
    ) -> toasty::Result<Db> {
        let mut builder = toasty::Db::builder();
        builder.models(models);

        // Set the table prefix
        builder.table_name_prefix(&self.isolate.table_prefix());

        // Apply caller customizations
        customize(&mut builder);

        // Always wrap with the instrumented test driver
        let instrumented_driver = InstrumentedDriver::new(self.setup.driver());
        self.handle = instrumented_driver.handle();

        // Build the database with the instrumented driver
        let db = builder.build(instrumented_driver).await?;
        db.push_schema().await?;

        for table in &db.schema().db.tables {
            self.tables.push(table.name.clone());
        }

        Ok(db)
    }

    /// Setup a database with models, always with logging enabled
    pub async fn setup_db(&mut self, models: ModelSet) -> Db {
        self.try_setup_db(models).await.unwrap()
    }

    /// Setup a database, applying the given customization to the
    /// [`toasty::db::Builder`] before building.
    pub async fn setup_db_with(
        &mut self,
        models: ModelSet,
        customize: impl FnOnce(&mut toasty::db::Builder),
    ) -> Db {
        self.try_setup_db_with(models, customize).await.unwrap()
    }

    /// Get the driver capability
    pub fn capability(&self) -> &'static toasty_core::driver::Capability {
        self.setup.driver().capability()
    }

    /// Get the instrumented-driver control handle. The handle exposes
    /// the operation log (for assertions) and fault injection.
    pub fn log(&self) -> &InstrumentedHandle {
        &self.handle
    }

    /// Queue a fault to fire on the next driver `exec` call. Faults
    /// fire in FIFO order. Only useful after `setup_db` has installed
    /// the instrumented driver.
    pub fn inject_fault(&self, fault: Fault) {
        self.handle.inject_fault(fault);
    }

    /// Set whether this test requires exclusive (serial) execution
    pub fn set_serial(&mut self, serial: bool) {
        self.serial = serial;
    }

    /// Run an async test function using the internal runtime
    pub fn run<R>(&mut self, f: impl AsyncFn(&mut Test) -> R)
    where
        R: Into<TestResult>,
    {
        // Acquire the appropriate lock: write lock for serial tests (exclusive),
        // read lock for normal tests (parallel).
        let _guard: Box<dyn std::any::Any> = if self.serial {
            Box::new(TEST_LOCK.write().unwrap_or_else(|e| e.into_inner()))
        } else {
            Box::new(TEST_LOCK.read().unwrap_or_else(|e| e.into_inner()))
        };

        // Temporarily take the runtime to avoid borrow checker issues
        let runtime = self.runtime.take().expect("runtime already consumed");
        let f: std::pin::Pin<Box<dyn std::future::Future<Output = R>>> = Box::pin(f(self));
        let result = runtime.block_on(f).into();

        // now, wut
        for table in &self.tables {
            runtime.block_on(self.setup.delete_table(table));
        }

        if let Some(error) = result.error {
            panic!("Driver test returned an error: {error}");
        }

        self.runtime = Some(runtime);
    }
}

pub struct TestResult {
    error: Option<Box<dyn Error>>,
}

impl From<()> for TestResult {
    fn from(_: ()) -> Self {
        TestResult { error: None }
    }
}

impl<O, E> From<Result<O, E>> for TestResult
where
    E: Into<Box<dyn Error>>,
{
    fn from(value: Result<O, E>) -> Self {
        TestResult {
            error: value.err().map(Into::into),
        }
    }
}
