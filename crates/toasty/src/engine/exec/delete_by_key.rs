use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    schema::db::TableId,
    stmt,
};

use crate::engine::exec::{Action, Output, VarId};

use super::{Exec, Result};

#[derive(Debug)]
pub(crate) struct DeleteByKey {
    /// Input variables. The first holds the list of keys to delete; `Arg`
    /// positions in `filter` and `condition` index into this list.
    pub input: Vec<VarId>,

    /// Where to store the output (impacted row count)
    pub output: Output,

    /// Which model to get
    pub table: TableId,

    /// Only delete keys that match the filter
    pub filter: Option<stmt::Expr>,

    /// Condition for optimistic locking (e.g., version check).
    pub condition: Option<stmt::Expr>,
}

impl Exec<'_> {
    pub(super) async fn action_delete_by_key(&mut self, action: &DeleteByKey) -> Result<()> {
        let (keys, filter, condition) = self
            .load_key_op_inputs(&action.input, &action.filter, &action.condition)
            .await?;

        let res = if keys.is_empty() {
            Rows::Count(0)
        } else {
            let mut total_count = 0u64;

            for key in keys {
                let op = operation::DeleteByKey {
                    table: action.table,
                    keys: vec![key],
                    filter: filter.clone(),
                    condition: condition.clone(),
                };

                let res = self.connection.exec(&self.engine.schema, op.into()).await?;

                match res.values {
                    Rows::Count(n) => total_count += n,
                    _ => panic!("expected Count from DeleteByKey"),
                }
            }

            Rows::Count(total_count)
        };

        self.vars.store(
            action.output.var,
            action.output.num_uses,
            ExecResponse::from_rows(res),
        );

        Ok(())
    }
}

impl From<DeleteByKey> for Action {
    fn from(src: DeleteByKey) -> Self {
        Self::DeleteByKey(src)
    }
}
