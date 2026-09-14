use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{exec, mir};

/// Executes a SQL statement against the database.
///
/// Used with SQL-capable drivers to delegate query execution to the database's
/// query engine. The statement may reference inputs from other nodes.
#[derive(Debug)]
pub(crate) struct ExecStatement {
    /// Nodes whose outputs are passed as arguments to the statement.
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The SQL statement to execute.
    pub(crate) stmt: stmt::Statement,

    /// The return type of this operation.
    pub(crate) ty: stmt::Type,

    /// How this statement's output is interpreted. For a conditional write the
    /// statement's leading two columns are probe counts checked against each
    /// other; see [`exec::ConditionalOutput`].
    pub(crate) conditional: exec::ConditionalOutput,

    /// Pagination configuration (None if not paginated)
    pub(crate) pagination: Option<exec::PaginationConfig>,
}

impl From<ExecStatement> for mir::Node {
    fn from(value: ExecStatement) -> Self {
        debug_assert!(
            {
                match &value.stmt {
                    stmt::Statement::Query(query) => !query.single,
                    _ => true,
                }
            },
            "as of now, no database can execute single queries"
        );

        mir::Operation::ExecStatement(Box::new(value)).into()
    }
}
