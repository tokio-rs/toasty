use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::{exec, mir, planner::VarTable};

#[derive(Debug)]
pub(crate) struct ExecStatement {
    /// Inputs needed to reify the statement
    pub(crate) inputs: IndexSet<mir::NodeId>,

    /// The database query to execute
    pub(crate) stmt: stmt::Statement,

    /// Node return type
    pub(crate) ty: stmt::Type,

    /// When true, the statement is a conditional update with no returning
    pub(crate) conditional_update_with_no_returning: bool,
}

impl ExecStatement {
    pub(crate) fn to_exec(
        &self,
        graph: &mir::Store,
        node: &mir::Node,
        var_table: &mut VarTable,
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
            .map(|input| graph[input].var.get().unwrap())
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
