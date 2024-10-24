use super::*;

impl<'stmt> Exec<'_, 'stmt> {
    pub(super) async fn exec_find_pk_by_index(
        &mut self,
        action: &plan::FindPkByIndex<'stmt>,
    ) -> Result<()> {
        let op = if !action.input.is_empty() {
            // TODO: this isn't actually right, but I'm temporarily hacking my
            // way through a bigger refactor. Hopefully nobody sees this
            // comment! J/K I'm sure this won't get fixed before I open up the
            // repo.
            assert_eq!(action.input.len(), 1);

            // Collect input
            let input = self.collect_input(&action.input[0]).await?;

            let mut args = [Some(sql::Expr::from(stmt::Value::List(input)))];

            let mut filter = action.filter.clone();
            filter.substitute(&mut args[..]);

            operation::FindPkByIndex {
                table: action.table,
                index: action.index,
                filter,
            }
        } else {
            action.apply()?
        };

        let res = self.db.driver.exec(&self.db.schema, op.into()).await?;

        self.vars.store(action.output, res);
        Ok(())
    }
}
