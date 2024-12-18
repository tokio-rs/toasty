use super::*;

impl Exec<'_> {
    pub(super) async fn exec_delete_by_key(&mut self, action: &plan::DeleteByKey) -> Result<()> {
        let args = if let Some(input) = &action.input {
            vec![self.collect_input(input).await?]
        } else {
            vec![]
        };

        let keys = match action.keys.eval(&args[..])? {
            stmt::Value::List(keys) => keys,
            res => todo!("res={res:#?}"),
        };

        if keys.is_empty() {
            return Ok(());
        } else {
            let op = operation::DeleteByKey {
                table: action.table,
                keys,
                filter: action.filter.clone(),
            };

            let res = self.db.driver.exec(&self.db.schema, op.into()).await?;
            assert!(res.rows.is_count(), "TODO");
        }

        Ok(())
    }
}
