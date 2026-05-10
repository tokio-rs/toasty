use async_trait::async_trait;
use std::{
    borrow::Cow,
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use toasty_core::{
    Result, Schema,
    driver::{Capability, Connection, Driver, ExecResponse, Operation, Rows},
    schema::db::{AppliedMigration, Migration, SchemaDiff},
};

/// A fault that can be injected into the next operation routed through
/// the driver. Faults are consumed in FIFO order: each `exec` call pops
/// at most one fault off the queue before delegating (or short-circuiting
/// past) the underlying driver.
#[derive(Debug, Clone)]
pub enum Fault {
    /// Causes the next `exec` to return `Error::connection_lost` without
    /// touching the underlying connection. The wrapping
    /// `LoggingConnection`'s `is_valid` flips to `false`, mirroring what
    /// a real connection-lost error would do and prompting the pool to
    /// evict the connection.
    ConnectionLost,
}

#[derive(Debug)]
pub struct LoggingDriver {
    inner: Box<dyn Driver>,

    /// Log of all operations executed through this driver.
    ops_log: Arc<Mutex<Vec<DriverOp>>>,

    /// Faults to inject on subsequent `exec` calls. Shared across all
    /// connections vended by this driver so a test can set up a fault
    /// before knowing which connection the pool will hand out.
    faults: Arc<Mutex<VecDeque<Fault>>>,
}

impl LoggingDriver {
    pub fn new(driver: Box<dyn Driver>) -> Self {
        Self {
            inner: driver,
            ops_log: Arc::new(Mutex::new(Vec::new())),
            faults: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Get a handle to access the operations log
    pub fn ops_log_handle(&self) -> Arc<Mutex<Vec<DriverOp>>> {
        self.ops_log.clone()
    }

    /// Get a handle to the fault-injection queue. Pushing a `Fault`
    /// onto this queue causes it to fire on the next `exec` call
    /// routed through any connection produced by this driver.
    pub fn faults_handle(&self) -> Arc<Mutex<VecDeque<Fault>>> {
        self.faults.clone()
    }
}

#[async_trait]
impl Driver for LoggingDriver {
    fn url(&self) -> Cow<'_, str> {
        self.inner.url()
    }

    fn capability(&self) -> &'static Capability {
        self.inner.capability()
    }

    async fn connect(&self) -> Result<Box<dyn Connection>> {
        Ok(Box::new(LoggingConnection {
            inner: self.inner.connect().await?,
            ops_log: self.ops_log_handle(),
            faults: self.faults_handle(),
            valid: AtomicBool::new(true),
        }))
    }

    fn generate_migration(&self, schema_diff: &SchemaDiff<'_>) -> Migration {
        self.inner.generate_migration(schema_diff)
    }

    async fn reset_db(&self) -> Result<()> {
        self.inner.reset_db().await
    }
}

#[derive(Debug)]
pub struct DriverOp {
    pub operation: Operation,
    pub response: ExecResponse,
}

/// A driver wrapper that logs all operations for testing purposes
#[derive(Debug)]
pub struct LoggingConnection {
    /// The underlying driver that actually executes operations
    inner: Box<dyn Connection>,

    /// Log of all operations executed through this driver
    /// Using Arc<Mutex> for thread-safe access from tests
    ops_log: Arc<Mutex<Vec<DriverOp>>>,

    /// Shared fault queue. See [`LoggingDriver::faults_handle`].
    faults: Arc<Mutex<VecDeque<Fault>>>,

    /// Set to `false` once an injected `ConnectionLost` fault has fired
    /// against this connection. Surfaced through [`Connection::is_valid`]
    /// so the pool evicts it the same way it would after a real
    /// connection-lost error.
    valid: AtomicBool,
}

#[async_trait]
impl Connection for LoggingConnection {
    async fn exec(&mut self, schema: &Arc<Schema>, operation: Operation) -> Result<ExecResponse> {
        // Pop a queued fault, if any, and short-circuit before reaching
        // the underlying driver.
        let fault = self
            .faults
            .lock()
            .expect("Failed to acquire faults lock")
            .pop_front();
        if let Some(fault) = fault {
            match fault {
                Fault::ConnectionLost => {
                    self.valid.store(false, Ordering::Release);
                    return Err(toasty_core::Error::connection_lost(std::io::Error::other(
                        "injected connection-lost fault",
                    )));
                }
            }
        }

        // Clone the operation for logging
        let operation_clone = operation.clone();

        // Execute the operation on the underlying driver
        let mut response = self.inner.exec(schema, operation).await?;

        // Duplicate the response for logging
        let duplicated_response = duplicate_response_mut(&mut response).await?;

        // Log the operation and response
        let driver_op = DriverOp {
            operation: operation_clone,
            response: duplicated_response,
        };

        self.ops_log
            .lock()
            .expect("Failed to acquire ops log lock")
            .push(driver_op);

        Ok(response)
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        self.inner.push_schema(schema).await
    }

    async fn applied_migrations(&mut self) -> Result<Vec<AppliedMigration>> {
        self.inner.applied_migrations().await
    }

    async fn apply_migration(&mut self, id: u64, name: &str, migration: &Migration) -> Result<()> {
        self.inner.apply_migration(id, name, migration).await
    }

    fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire) && self.inner.is_valid()
    }
}

/// Duplicate an ExecResponse, using ValueStream::dup() for value streams
/// This version takes a mutable reference so we can call dup() on the ValueStream
async fn duplicate_response_mut(response: &mut ExecResponse) -> Result<ExecResponse> {
    let values = match &mut response.values {
        Rows::Count(count) => Rows::Count(*count),
        Rows::Value(_) => todo!(),
        Rows::Stream(stream) => {
            // Duplicate the value stream
            let duplicated_stream = stream.dup().await?;
            Rows::Stream(duplicated_stream)
        }
    };

    Ok(ExecResponse::from_rows(values))
}
