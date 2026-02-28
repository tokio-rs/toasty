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

    /// Maximum number of items to evaluate (maps to DynamoDB `Limit`).
    pub limit: Option<i64>,

    /// Sort key ordering (`true` = ascending, `false` = descending).
    pub scan_index_forward: Option<bool>,

    /// Cursor for resuming a paginated query (maps to DynamoDB
    /// `ExclusiveStartKey`).
    pub exclusive_start_key: Option<stmt::Value>,
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
        if let Some((items, pred_template)) = super::try_extract_any_map_list(&pk_filter) {
            let items = items.to_vec();
            let pred_template = pred_template.clone();

            let mut all_rows: Vec<stmt::Value> = Vec::new();

            for item in items {
                let mut per_call_filter = pred_template.clone();
                // Mirror simplify_expr_any: unpack Record fields so arg(i) binds to field i.
                match item {
                    stmt::Value::Record(r) => per_call_filter.substitute(&r.fields[..]),
                    item => per_call_filter.substitute([item]),
                }

                let res = self
                    .connection
                    .exec(
                        &self.engine.schema,
                        operation::QueryPk {
                            table: action.table,
                            index: action.index,
                            select: action.columns.clone(),
                            pk_filter: per_call_filter,
                            filter: action.row_filter.clone(),
                            limit: action.limit,
                            scan_index_forward: action.scan_index_forward,
                            exclusive_start_key: action.exclusive_start_key.clone(),
                        }
                        .into(),
                    )
                    .await?;

                all_rows.extend(res.rows.into_value_stream().collect().await?);
            }

            debug_assert!(
                {
                    let mut seen = std::collections::HashSet::new();
                    all_rows.iter().all(|row| seen.insert(row))
                },
                "fan-out produced duplicate rows in QueryPk"
            );

            self.vars.store(
                action.output.var,
                action.output.num_uses,
                Rows::Stream(stmt::ValueStream::from_vec(all_rows)),
            );
            return Ok(());
        }

        if action.input.is_some() {
            simplify::simplify_expr(self.engine.expr_cx(), &mut pk_filter);
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
                    limit: action.limit,
                    scan_index_forward: action.scan_index_forward,
                    exclusive_start_key: action.exclusive_start_key.clone(),
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
