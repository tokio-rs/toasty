use toasty_core::driver::{ExecResponse, Rows, operation};

use crate::engine::{exec::Exec, mir};

use super::Result;

impl Exec<'_> {
    pub(super) async fn exec_delete_by_key(
        &mut self,
        action: &mir::DeleteByKey,
    ) -> Result<ExecResponse> {
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
                    condition: action.condition.clone(),
                };

                let res = self.connection.exec(&self.engine.schema, op.into()).await?;

                match res.values {
                    Rows::Count(n) => total_count += n,
                    _ => panic!("expected Count from DeleteByKey"),
                }
            }

            Rows::Count(total_count)
        };

        Ok(ExecResponse::from_rows(res))
    }
}
