use toasty_core::{
    driver::{operation, Rows},
    schema::db::{IndexId, TableId},
    stmt,
};

use crate::{
    engine::exec::{Action, Exec, Output, VarId},
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

        let filters = self.split_filter(filter, action.table);
        let mut all_rows = Vec::new();

        for f in filters {
            let res = self
                .connection
                .exec(
                    &self.engine.schema,
                    operation::FindPkByIndex {
                        table: action.table,
                        index: action.index,
                        filter: f,
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

        Ok(())
    }
}

impl From<FindPkByIndex> for Action {
    fn from(src: FindPkByIndex) -> Self {
        Self::FindPkByIndex(src)
    }
}
