use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::mir;

/// Performs an optimistic read-modify-write operation.
///
/// Used for conditional updates where the write only succeeds if the values
/// read have not been modified since reading. This is a fallback for databases
/// that do not support conditional updates in a single statement (e.g., SQLite,
/// MySQL without CTE support).
#[derive(Debug)]
pub(crate) struct ReadModifyWrite {
    /// Nodes providing input arguments for the statements.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The read query that fetches current values.
    pub(crate) read: stmt::Query,

    /// The write statement to execute if the condition holds.
    pub(crate) write: stmt::Statement,

    /// The return type. When the write carries a `RETURNING`, this is
    /// `List<Record>`; without one the type is `Unit` and the write reports
    /// only a row count.
    pub(crate) ty: stmt::Type,
}

impl From<ReadModifyWrite> for mir::Node {
    fn from(value: ReadModifyWrite) -> Self {
        mir::Operation::ReadModifyWrite(Box::new(value)).into()
    }
}
