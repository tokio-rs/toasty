use crate::logging_driver::DriverOp;
use std::sync::{Arc, Mutex};
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

    /// Check if any operation matches the given predicate
    pub fn any<F>(&self, predicate: F) -> bool
    where
        F: Fn(&Operation) -> bool,
    {
        self.ops
            .lock()
            .unwrap()
            .iter()
            .any(|op| predicate(&op.operation))
    }

    /// Count operations matching the given predicate
    pub fn count<F>(&self, predicate: F) -> usize
    where
        F: Fn(&Operation) -> bool,
    {
        self.ops
            .lock()
            .unwrap()
            .iter()
            .filter(|op| predicate(&op.operation))
            .count()
    }

    /// Check if there's an Insert operation
    pub fn has_insert(&self) -> bool {
        self.any(|op| matches!(op, Operation::Insert(_)))
    }

    /// Check if there's a GetByKey operation
    pub fn has_get_by_key(&self) -> bool {
        self.any(|op| matches!(op, Operation::GetByKey(_)))
    }

    /// Check if there's an UpdateByKey operation
    pub fn has_update_by_key(&self) -> bool {
        self.any(|op| matches!(op, Operation::UpdateByKey(_)))
    }

    /// Check if there's a DeleteByKey operation
    pub fn has_delete_by_key(&self) -> bool {
        self.any(|op| matches!(op, Operation::DeleteByKey(_)))
    }

    /// Check if there's a QuerySql operation
    pub fn has_query_sql(&self) -> bool {
        self.any(|op| matches!(op, Operation::QuerySql(_)))
    }

    /// Check if there's a QueryPk operation
    pub fn has_query_pk(&self) -> bool {
        self.any(|op| matches!(op, Operation::QueryPk(_)))
    }

    /// Clear the log
    pub fn clear(&mut self) {
        self.ops.lock().unwrap().clear();
    }

    /// Remove and return the first operation from the log
    /// Returns None if the log is empty
    pub fn pop(&mut self) -> Option<(Operation, Response)> {
        let mut ops = self.ops.lock().unwrap();
        if ops.is_empty() {
            None
        } else {
            let driver_op = ops.remove(0);
            Some((driver_op.operation, driver_op.response))
        }
    }

    /// Get access to all operations for custom assertions
    /// This is an escape hatch for complex assertions
    pub fn with_ops<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&[DriverOp]) -> R,
    {
        let ops = self.ops.lock().unwrap();
        f(&ops)
    }
}
