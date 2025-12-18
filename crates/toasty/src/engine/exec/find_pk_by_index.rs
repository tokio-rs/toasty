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

        // Collect input values and substitute into the statement
        if !action.input.is_empty() {
            assert!(action.input.len() == 1);
            let input = self.collect_input(&action.input).await?;

            filter.substitute(&input);

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
                &self.engine.schema.db,
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
