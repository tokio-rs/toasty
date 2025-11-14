use indexmap::IndexSet;
use toasty_core::{
    schema::db::{ColumnId, TableId},
    stmt,
};

use crate::engine::{exec, mir};

#[derive(Debug)]
pub(crate) struct GetByKey {
    /// Keys are always specified as an input, whether const or a set of
    /// dependent materializations and transformations.
    pub(crate) input: mir::NodeId,

    /// The table to get keys from
    pub(crate) table: TableId,

    /// Columns to get
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// Return type
    pub(crate) ty: stmt::Type,
}

impl GetByKey {
    pub(crate) fn to_exec(
        &self,
        graph: &mir::Store,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::GetByKey {
        let input = graph.var_id(self.input);

        let output = var_table.register_var(node.ty().clone());
        node.var.set(Some(output));

        let columns = self
            .columns
            .iter()
            .map(|expr_reference| {
                let stmt::ExprReference::Column(expr_column) = expr_reference else {
                    todo!()
                };
                debug_assert_eq!(expr_column.nesting, 0);
                debug_assert_eq!(expr_column.table, 0);

                ColumnId {
                    table: self.table,
                    index: expr_column.column,
                }
            })
            .collect();

        exec::GetByKey {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            columns,
        }
    }
}

impl From<GetByKey> for mir::Node {
    fn from(value: GetByKey) -> Self {
        mir::Operation::GetByKey(value).into()
    }
}
