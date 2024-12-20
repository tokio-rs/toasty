use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn exec_query_pk(&mut self, action: &plan::QueryPk) -> Result<()> {
        let op = action.apply()?;
        let res = self
            .db
            .driver
            .exec(
                &self.db.schema,
                operation::QueryPk {
                    table: action.table,
                    select: action.columns.clone(),
                    pk_filter: action.pk_filter.clone(),
                    filter: action.filter.clone(),
                }
                .into(),
            )
            .await?;

        let rows = match res.rows {
            Rows::Values(rows) => rows,
            _ => todo!("res={res:#?}"),
        };

        // TODO: don't clone
        let project = action.output.project.clone();
        let post_filter = action.post_filter.clone();

        self.vars.store(
            action.output.var,
            ValueStream::from_stream(async_stream::try_stream! {
                for await value in rows {
                    let args = [value?];

                    let select = if let Some(filter) = &post_filter {
                        filter.eval_bool(&args)?
                    } else {
                        true
                    };

                    if select {
                        let value = if project.is_identity() {
                            let [value] = args else { todo!() };
                            value
                        } else {
                            project.eval(&args)?
                        };

                        yield value;
                    }
                }
            }),
        );

        Ok(())
    }
}
