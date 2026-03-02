use toasty_core::{
    driver::{operation, Rows},
    schema::db::{IndexId, TableId},
    stmt,
};

use crate::{
    engine::{
        exec::{Action, Exec, Output, VarId},
        simplify,
    },
    Result,
};

/// Schema: `self` references [index-fields, input-fields] flattened
#[derive(Debug)]
pub(crate) struct FindPkByIndex {
    /// How to access input from the variable table.
    pub input: Vec<VarId>,

    /// Where to store the output
    pub output: Output,

    /// Table to query
    pub table: TableId,

    /// Index to use
    pub index: IndexId,

    /// Filter to apply to index
    pub filter: stmt::Expr,
}

impl Exec<'_> {
    pub(super) async fn action_find_pk_by_index(&mut self, action: &FindPkByIndex) -> Result<()> {
        let mut filter = action.filter.clone();

        if !action.input.is_empty() {
            assert!(action.input.len() == 1);
            let input = self.collect_input(&action.input).await?;
            filter.substitute(&input);
        }

        // Fan-out for ANY(MAP(Value::List([...]), pred)) — one driver call per element.
        // Must be checked BEFORE simplify_expr, which would re-expand the list to Expr::Or.
        // Handles both constant lists (plan-time OR rewrite) and post-input-substitution lists.
        if let Some((items, pred_template)) = super::try_extract_any_map_list(&filter) {
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
                        operation::FindPkByIndex {
                            table: action.table,
                            index: action.index,
                            filter: per_call_filter,
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
                "fan-out produced duplicate rows in FindPkByIndex"
            );

            self.vars.store(
                action.output.var,
                action.output.num_uses,
                Rows::Stream(stmt::ValueStream::from_vec(all_rows)),
            );
            return Ok(());
        }

        if !action.input.is_empty() {
            simplify::simplify_expr(self.engine.expr_cx(), &mut filter);
        }

        if filter.is_false() {
            let rows = Rows::Stream(stmt::ValueStream::default());
            self.vars
                .store(action.output.var, action.output.num_uses, rows);
            return Ok(());
        }

        let res = self
            .connection
            .exec(
                &self.engine.schema,
                operation::FindPkByIndex {
                    table: action.table,
                    index: action.index,
                    filter,
                }
                .into(),
            )
            .await?;

        self.vars
            .store(action.output.var, action.output.num_uses, res.rows);

        Ok(())
    }
}

impl From<FindPkByIndex> for Action {
    fn from(src: FindPkByIndex) -> Self {
        Self::FindPkByIndex(src)
    }
}
