use crate::logging_driver::DriverOp;
use std::{
    fmt,
    sync::{Arc, Mutex},
};
use toasty_core::driver::{Operation, Response};

/// A wrapper around the operations log that provides a clean API for tests
pub struct ExecLog {
    ops: Arc<Mutex<Vec<DriverOp>>>,
}

impl ExecLog {
    pub(crate) fn new(ops: Arc<Mutex<Vec<DriverOp>>>) -> Self {
        Self { ops }
    }

    /// Get the number of logged operations
    pub fn len(&self) -> usize {
        self.ops.lock().unwrap().len()
    }

    /// Check if the log is empty
    pub fn is_empty(&self) -> bool {
        self.ops.lock().unwrap().is_empty()
    }

    /// Clear the log
    pub fn clear(&mut self) {
        self.ops.lock().unwrap().clear();
    }

    /// Remove and return the first operation from the log
    /// Returns None if the log is empty
    #[track_caller]
    pub fn pop(&mut self) -> (Operation, Response) {
        let mut ops = self.ops.lock().unwrap();
        if ops.is_empty() {
            panic!("no operations in log");
        } else {
            let driver_op = ops.remove(0);
            (driver_op.operation, driver_op.response)
        }
    }
}

impl fmt::Debug for ExecLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ops = self.ops.lock().unwrap();
        f.debug_struct("ExecLog").field("ops", &*ops).finish()
    }
}
