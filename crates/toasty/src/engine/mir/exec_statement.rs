use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{
    exec,
    mir::{self, LogicalPlan},
};

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

    /// When `true`, this is a conditional update that returns status, not rows.
    pub(crate) conditional_update_with_no_returning: bool,
}

impl ExecStatement {
    pub(crate) fn to_exec(
        &self,
        logical_plan: &LogicalPlan,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::ExecStatement {
        debug_assert!(
            {
                match &self.stmt {
                    stmt::Statement::Query(query) => !query.single,
                    _ => true,
                }
            },
            "as of now, no database can execute single queries"
        );

        let input_vars = self
            .inputs
            .iter()
            .map(|input| logical_plan[input].var.get().unwrap())
            .collect();

        let var = var_table.register_var(self.ty.clone());
        node.var.set(Some(var));

        let output_ty = match &self.ty {
            stmt::Type::List(ty_rows) => {
                let ty_fields = match &**ty_rows {
                    stmt::Type::Record(ty_fields) => ty_fields.clone(),
                    _ => todo!("ty={:#?}; node={node:#?}", self.ty),
                };

                Some(ty_fields)
            }
            stmt::Type::Unit => None,
            _ => todo!("ty={:#?}", self.ty),
        };

        exec::ExecStatement {
            input: input_vars,
            output: exec::ExecStatementOutput {
                ty: output_ty,
                output: exec::Output {
                    var,
                    num_uses: node.num_uses.get(),
                },
            },
            stmt: self.stmt.clone(),
            conditional_update_with_no_returning: self.conditional_update_with_no_returning,
        }
    }
}

impl From<ExecStatement> for mir::Node {
    fn from(value: ExecStatement) -> Self {
        mir::Operation::ExecStatement(Box::new(value)).into()
    }
}
