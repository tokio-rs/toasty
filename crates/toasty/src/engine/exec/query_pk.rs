use crate::{
    engine::{
        exec::{Action, Exec, Output, VarId},
        simplify,
    },
    Result,
};
use toasty_core::{
    driver::{operation, Rows},
    schema::db::{ColumnId, IndexId, TableId},
    stmt,
};

#[derive(Debug)]
pub(crate) struct QueryPk {
    /// Where to get the input
    pub input: Option<VarId>,

    /// Where to store the result
    pub output: Output,

    /// Table to query
    pub table: TableId,

    /// Optional index to query. None = primary key, Some(id) = secondary index
    pub index: Option<IndexId>,

    /// Columns to get
    pub columns: Vec<ColumnId>,

    /// How to filter the index.
    pub pk_filter: stmt::Expr,

    /// Filter to pass to the database
    pub row_filter: Option<stmt::Expr>,
}

impl Exec<'_> {
    pub(super) async fn action_query_pk(&mut self, action: &QueryPk) -> Result<()> {
        let mut pk_filter = action.pk_filter.clone();

        if let Some(input) = &action.input {
            let input = self.collect_input(&[*input]).await?;
            pk_filter.substitute(&input);
        }

        // Fan-out for ANY(MAP(Value::List([...]), pred)) — one driver call per element.
        // Must be checked BEFORE simplify_expr, which would re-expand the list to Expr::Or.
        // Handles both constant lists (plan-time OR rewrite) and post-input-substitution lists.
        if let Some(all_rows) = self
            .try_fan_out(&pk_filter, action.table, |per_call_filter| {
                operation::QueryPk {
                    table: action.table,
                    index: action.index,
                    select: action.columns.clone(),
                    pk_filter: per_call_filter,
                    filter: action.row_filter.clone(),
                }
                .into()
            })
            .await?
        {
            self.vars.store(
                action.output.var,
                action.output.num_uses,
                Rows::Stream(stmt::ValueStream::from_vec(all_rows)),
            );
            return Ok(());
        }

        {
            let table = self.engine.schema.db.table(action.table);
            simplify::simplify_expr(self.engine.expr_cx_for(table), &mut pk_filter);
        }

        // An unsatisfiable filter (null or false) means no rows can match
        // (e.g. null FK from optional belongs_to).
        if pk_filter.is_unsatisfiable() {
            self.vars.store(
                action.output.var,
                action.output.num_uses,
                Rows::Stream(stmt::ValueStream::default()),
            );
            return Ok(());
        }

        let res = self
            .connection
            .exec(
                &self.engine.schema,
                operation::QueryPk {
                    table: action.table,
                    index: action.index,
                    select: action.columns.clone(),
                    pk_filter,
                    filter: action.row_filter.clone(),
                }
                .into(),
            )
            .await?;

        self.vars
            .store(action.output.var, action.output.num_uses, res.rows);
        Ok(())
    }
}

impl From<QueryPk> for Action {
    fn from(value: QueryPk) -> Self {
        Action::QueryPk(value)
    }
}
