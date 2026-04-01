use toasty_core::{
    driver::{ExecResponse, Rows, operation},
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
            .values
            .collect_as_value()
            .await?
            .into_list_unwrap();

        let res = if keys.is_empty() {
            Rows::Count(0)
        } else {
            let mut total_count = 0u64;

            for key in keys {
                let op = operation::DeleteByKey {
                    table: action.table,
                    keys: vec![key],
                    filter: action.filter.clone(),
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
