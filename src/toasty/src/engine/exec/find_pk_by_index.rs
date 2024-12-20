use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn exec_find_pk_by_index(
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
                &self.db.schema,
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

        if action.output.project.is_identity() {
            self.vars.store(action.output.var, rows);
        } else {
            let project = action.output.project.clone();

            self.vars.store(
                action.output.var,
                ValueStream::from_stream(async_stream::try_stream! {
                    for await value in rows {
                        let value = value?;
                        let value = project.eval(&[value])?;
                        yield value;
                    }
                }),
            );
        }

        Ok(())
    }
}
