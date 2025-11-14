use indexmap::IndexSet;
use toasty_core::{
    schema::db::{ColumnId, TableId},
    stmt,
};

use crate::engine::{exec, mir};

#[derive(Debug)]
pub(crate) struct QueryPk {
    pub(crate) input: Option<mir::NodeId>,

    pub(crate) table: TableId,

    /// Columns to get
    pub(crate) columns: IndexSet<stmt::ExprReference>,

    /// How to filter the index
    pub(crate) pk_filter: stmt::Expr,

    /// Additional filter to pass to the database
    pub(crate) row_filter: Option<stmt::Expr>,

    pub(crate) ty: stmt::Type,
}

impl QueryPk {
    pub(crate) fn to_exec(
        &self,
        graph: &mir::Store,
        node: &mir::Node,
        var_table: &mut exec::VarDecls,
    ) -> exec::QueryPk {
        let input = self.input.map(|node_id| graph.var_id(node_id));
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

        exec::QueryPk {
            input,
            output: exec::Output {
                var: output,
                num_uses: node.num_uses.get(),
            },
            table: self.table,
            columns,
            pk_filter: self.pk_filter.clone(),
            row_filter: self.row_filter.clone(),
        }
    }
}

impl From<QueryPk> for mir::Node {
    fn from(value: QueryPk) -> Self {
        mir::Operation::QueryPk(value).into()
    }
}
