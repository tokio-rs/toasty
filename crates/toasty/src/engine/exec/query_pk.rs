use crate::{
    engine::{
        exec::{Action, Exec, Output, VarId},
        simplify,
    },
    Result,
};
use toasty_core::{
    driver::{operation, Rows},
    schema::db::{ColumnId, TableId},
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

            // Fan-out for ANY(MAP(Value::List([...]), pred)) — one driver call per element.
            // Must be checked BEFORE simplify_expr, which would re-expand the list to Expr::Or.
            if let Some((items, pred_template)) = super::try_extract_any_map_list(&pk_filter) {
                let items = items.to_vec();
                let pred_template = pred_template.clone();

                let mut all_rows: Vec<stmt::Value> = Vec::new();

                for item in items {
                    let mut per_call_filter = pred_template.clone();
                    per_call_filter.substitute(&vec![item]);

                    let res = self
                        .connection
                        .exec(
                            &self.engine.schema.db,
                            operation::QueryPk {
                                table: action.table,
                                select: action.columns.clone(),
                                pk_filter: per_call_filter,
                                filter: action.row_filter.clone(),
                            }
                            .into(),
                        )
                        .await?;

                    all_rows.extend(res.rows.into_value_stream().collect().await?);
                }

                self.vars.store(
                    action.output.var,
                    action.output.num_uses,
                    Rows::Stream(stmt::ValueStream::from_vec(all_rows)),
                );
                return Ok(());
            }

            simplify::simplify_expr(self.engine.expr_cx(), &mut pk_filter);
        }

        let res = self
            .connection
            .exec(
                &self.engine.schema.db,
                operation::QueryPk {
                    table: action.table,
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
