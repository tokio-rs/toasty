use toasty_core::driver::operation;

use super::{plan, Exec, Result};
use crate::engine::simplify;

impl Exec<'_> {
    pub(super) async fn action_find_pk_by_index2(
        &mut self,
        action: &plan::FindPkByIndex2,
    ) -> Result<()> {
        let mut filter = action.filter.clone();

        // Collect input values and substitute into the statement
        if !action.input.is_empty() {
            // Only one input supported so far
            assert!(action.input.len() == 1, "TODO");
            let input = self.collect_input2(&action.input).await?;

            println!("filter={filter:#?}; input={input:#?}");
            filter.substitute(&input);

            simplify::simplify_expr(self.engine.expr_cx(), &mut filter);
        }

        let res = self
            .engine
            .driver
            .exec(
                &self.engine.schema.db,
                operation::FindPkByIndex {
                    table: action.table,
                    index: action.index,
                    filter,
                }
                .into(),
            )
            .await?;

        self.vars
            .store(action.output.var, action.output.num_uses, res.rows);

        Ok(())
    }
}
