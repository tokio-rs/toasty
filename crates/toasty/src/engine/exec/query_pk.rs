use crate::{
    engine::{exec::Exec, plan, simplify},
    Result,
};
use toasty_core::driver::operation;

impl Exec<'_> {
    pub(super) async fn action_query_pk(&mut self, action: &plan::QueryPk) -> Result<()> {
        let mut pk_filter = action.pk_filter.clone();

        if let Some(input) = &action.input {
            let input = self.collect_input2(&[*input]).await?;
            pk_filter.substitute(&input);
            simplify::simplify_expr(self.engine.expr_cx(), &mut pk_filter);
        }

        let res = self
            .engine
            .driver
            .exec(
                &self.engine.schema.db,
                operation::QueryPk {
                    table: action.table,
                    select: action.columns.clone(),
                    pk_filter,
                    filter: action.row_filter.clone(),
                }
                .into(),
            )
            .await?;

        self.vars
            .store(action.output.var, action.output.num_uses, res.rows);
        Ok(())
    }
}
