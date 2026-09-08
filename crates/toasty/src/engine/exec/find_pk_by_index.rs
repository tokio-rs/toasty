use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    stmt,
};

use crate::{
    Result,
    engine::{exec::Exec, mir},
};

impl Exec<'_> {
    pub(super) async fn exec_find_pk_by_index(
        &mut self,
        action: &mir::FindPkByIndex,
    ) -> Result<ExecResponse> {
        let mut filter = action.filter.clone();

        if !action.inputs.is_empty() {
            assert!(action.inputs.len() == 1);
            let input = self.collect_input(action.inputs.iter().copied()).await?;
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

            all_rows.extend(res.values.into_value_stream().collect().await?);
        }

        Ok(ExecResponse::from_rows(Rows::Stream(
            stmt::ValueStream::from_vec(all_rows),
        )))
    }
}
