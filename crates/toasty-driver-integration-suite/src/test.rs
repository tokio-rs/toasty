use std::sync::{Arc, Mutex};

use toasty::Db;
use tokio::runtime::Runtime;

use crate::{ExecLog, Isolate, LoggingDriver, Setup};

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

    exec_log: ExecLog,

    /// List of all tables created during the test. These will need to be removed later.
    tables: Vec<String>,

    /// Cached driver capability
    capability: &'static toasty::driver::Capability,
}

impl Test {
    pub fn new(setup: Arc<dyn Setup>) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create Tokio runtime");

        // Get capability early
        let driver = setup.driver();
        let capability = runtime.block_on(async {
            let conn = driver.connect().await.expect("failed to connect");
            conn.capability()
        });

        Test {
            setup,
            isolate: Isolate::new(),
            runtime: Some(runtime),
            exec_log: ExecLog::new(Arc::new(Mutex::new(Vec::new()))),
            tables: vec![],
            capability,
        }
    }

    /// Try to setup a database with models, returns Result for error handling
    pub async fn try_setup_db(&mut self, mut builder: toasty::db::Builder) -> toasty::Result<Db> {
        // Set the table prefix
        builder.table_name_prefix(&self.isolate.table_prefix());

        // Always wrap with logging
        let logging_driver = LoggingDriver::new(self.setup.driver());
        let ops_log = logging_driver.ops_log_handle();
        self.exec_log = ExecLog::new(ops_log);

        // Build the database with the logging driver
        let db = builder.build(logging_driver).await?;
        db.reset_db().await?;

        for table in &db.schema().db.tables {
            self.tables.push(table.name.clone());
        }

        Ok(db)
    }

    /// Setup a database with models, always with logging enabled
    pub async fn setup_db(&mut self, builder: toasty::db::Builder) -> Db {
        self.try_setup_db(builder).await.unwrap()
    }

    /// Get the driver capability
    pub fn capability(&self) -> &'static toasty::driver::Capability {
        self.capability
    }

    /// Get the execution log for assertions
    pub fn log(&mut self) -> &mut ExecLog {
        &mut self.exec_log
    }

    /// Run an async test function using the internal runtime
    pub fn run(&mut self, f: impl AsyncFn(&mut Test)) {
        // Temporarily take the runtime to avoid borrow checker issues
        let runtime = self.runtime.take().expect("runtime already consumed");
        let f: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> = Box::pin(f(self));
        runtime.block_on(f);

        // now, wut
        for table in &self.tables {
            runtime.block_on(self.setup.delete_table(table));
        }

        self.runtime = Some(runtime);
    }
}
