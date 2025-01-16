use super::*;

use crate::driver::Rows;

impl Exec<'_> {
    pub(super) async fn action_query_pk(&mut self, action: &plan::QueryPk) -> Result<()> {
        let res = self
            .db
            .driver
            .exec(
                &self.db.schema.db,
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

        let res = self.project_and_filter_output(
            rows,
            &action.output.project,
            action.post_filter.as_ref(),
        );

        self.vars.store(action.output.var, res);

        Ok(())
    }
}
