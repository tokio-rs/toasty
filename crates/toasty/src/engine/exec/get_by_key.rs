use crate::{
    engine::exec::{Action, Exec, Output, VarId},
    Result,
};
use toasty_core::{
    driver::{operation, Rows},
    schema::db::{ColumnId, TableId},
    stmt::ValueStream,
};

/// Get a model by key
#[derive(Debug)]
pub(crate) struct GetByKey {
    /// Where to get the keys to load
    pub input: VarId,

    /// Where to store the result
    pub output: Output,

    /// Table to query
    pub table: TableId,

    /// Columns to get
    pub columns: Vec<ColumnId>,
}

impl Exec<'_> {
    pub(super) async fn action_get_by_key(&mut self, action: &GetByKey) -> Result<()> {
        let keys = self
            .vars
            .load(action.input)
            .await?
            .collect_as_value()
            .await?
            .unwrap_list();

        let res = if keys.is_empty() {
            Rows::value_stream(ValueStream::default())
        } else {
            let op = operation::GetByKey {
                table: action.table,
                select: action.columns.clone(),
                keys,
            };

            let res = self
                .connection
                .exec(&self.engine.schema.db, op.into())
                .await?;
            res.rows
        };

        self.vars
            .store(action.output.var, action.output.num_uses, res);
        Ok(())
    }
}

impl From<GetByKey> for Action {
    fn from(src: GetByKey) -> Self {
        Self::GetByKey(src)
    }
}
