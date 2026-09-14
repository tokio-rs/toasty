use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::{
    driver::{ExecResponse, Rows, operation},
    stmt::{ValueSet, ValueStream},
};

impl Exec<'_> {
    pub(super) async fn exec_get_by_key(&mut self, action: &mir::GetByKey) -> Result<ExecResponse> {
        let mut seen = ValueSet::new();
        let keys: Vec<_> = self
            .vars
            .load(action.input)
            .await?
            .values
            .collect_as_value()
            .await?
            .into_list_unwrap()
            .into_iter()
            .filter(|k| !k.is_null())
            .filter(|k| seen.insert(k.clone()))
            .collect();

        let res = if keys.is_empty() {
            Rows::value_stream(ValueStream::default())
        } else {
            let op = operation::GetByKey {
                table: action.table,
                select: mir::column_ids(action.table, &action.columns).collect(),
                keys,
            };

            let res = self.connection.exec(&self.engine.schema, op.into()).await?;
            res.values
        };

        Ok(ExecResponse::from_rows(res))
    }
}
