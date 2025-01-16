use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn action_find_pk_by_index(
        &mut self,
        action: &plan::FindPkByIndex,
    ) -> Result<()> {
        let mut filter = action.filter.clone();

        if let Some(input) = &action.input {
            let input = self.collect_input(input).await?;

            filter.substitute(&[input]);

            simplify::simplify_expr(&self.db.schema, simplify::ExprTarget::Const, &mut filter);
        }

        let res = self
            .db
            .driver
            .exec(
                &self.db.schema.db,
                operation::FindPkByIndex {
                    table: action.table,
                    index: action.index,
                    filter,
                }
                .into(),
            )
            .await?;

        let rows = match res.rows {
            Rows::Values(values) => values,
            Rows::Count(_) => todo!(),
        };

        let res = self.project_and_filter_output(rows, &action.output.project, None);
        self.vars.store(action.output.var, res);

        Ok(())
    }
}
