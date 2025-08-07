use std::sync::{Arc, Mutex};
use toasty_core::{
    async_trait,
    driver::{Capability, Driver, Operation, Response, Rows},
    schema::db::Schema,
    Result,
};

#[derive(Debug)]
pub struct DriverOp {
    pub operation: Operation,
    pub response: Response,
}

/// A driver wrapper that logs all operations for testing purposes
#[derive(Debug)]
pub struct LoggingDriver {
    /// The underlying driver that actually executes operations
    inner: Box<dyn Driver>,

    /// Log of all operations executed through this driver
    /// Using Arc<Mutex> for thread-safe access from tests
    ops_log: Arc<Mutex<Vec<DriverOp>>>,
}

impl LoggingDriver {
    pub fn new(inner: Box<dyn Driver>) -> Self {
        Self {
            inner,
            ops_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get a handle to access the operations log
    pub fn ops_log_handle(&self) -> Arc<Mutex<Vec<DriverOp>>> {
        self.ops_log.clone()
    }
}

#[async_trait]
impl Driver for LoggingDriver {
    fn capability(&self) -> &Capability {
        self.inner.capability()
    }

    async fn register_schema(&mut self, schema: &Schema) -> Result<()> {
        self.inner.register_schema(schema).await
    }

    async fn exec(&self, schema: &Arc<Schema>, operation: Operation) -> Result<Response> {
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

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        self.inner.reset_db(schema).await
    }
}

/// Duplicate a Response, using ValueStream::dup() for value streams
/// This version takes a mutable reference so we can call dup() on the ValueStream
async fn duplicate_response_mut(response: &mut Response) -> Result<Response> {
    let rows = match &mut response.rows {
        Rows::Count(count) => Rows::Count(*count),
        Rows::Values(stream) => {
            // Duplicate the value stream
            let duplicated_stream = stream.dup().await?;
            Rows::Values(duplicated_stream)
        }
    };

    Ok(Response { rows })
}
