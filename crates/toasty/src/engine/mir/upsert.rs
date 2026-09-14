use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::mir;

/// Executes a single-row upsert on a non-SQL database.
///
/// The planner emits this operation after lowering the conflict target and
/// verifying the requested upsert behavior against the driver's capabilities.
#[derive(Debug)]
pub(crate) struct Upsert {
    /// Nodes whose outputs are passed as arguments to the statement.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The lowered insert and conflict action.
    pub(crate) stmt: stmt::Insert,

    /// The return type of this operation.
    pub(crate) ty: stmt::Type,
}

impl From<Upsert> for mir::Node {
    fn from(value: Upsert) -> Self {
        mir::Operation::Upsert(Box::new(value)).into()
    }
}
