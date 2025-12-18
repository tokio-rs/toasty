use toasty_core::{
    driver::{operation, Rows},
    schema::db::TableId,
    stmt,
};

use crate::engine::exec::{Action, Output, VarId};

use super::{Exec, Result};

/// Input is the key to delete
#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// How to access input from the variable table.
    pub input: VarId,

    /// Where to store the output (impacted row count)
    pub output: Output,

    /// Which model to get
    pub table: TableId,

    /// Only delete keys that match the filter
    pub filter: Option<stmt::Expr>,
}

impl Exec<'_> {
    pub(super) async fn action_delete_by_key(&mut self, action: &DeleteByKey) -> Result<()> {
        let keys = self
            .vars
            .load(action.input)
            .await?
            .collect_as_value()
            .await?
            .unwrap_list();

        let res = if keys.is_empty() {
            Rows::Count(0)
        } else {
            let op = operation::DeleteByKey {
                table: action.table,
                keys,
                filter: action.filter.clone(),
            };

            let res = self
                .connection
                .exec(&self.engine.schema.db, op.into())
                .await?;

            assert!(res.rows.is_count(), "TODO");
            res.rows
        };

        self.vars
            .store(action.output.var, action.output.num_uses, res);

        Ok(())
    }
}

impl From<DeleteByKey> for Action {
    fn from(src: DeleteByKey) -> Self {
        Self::DeleteByKey(src)
    }
}
