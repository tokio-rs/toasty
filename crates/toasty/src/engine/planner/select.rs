use super::{plan, Planner, Result};
use toasty_core::stmt;

impl Planner<'_> {
    pub(super) fn plan_stmt_select(&mut self, stmt: stmt::Query) -> Result<plan::VarId> {
        // TODO: don't unwrap once vars can store more than just ValueStream
        let var = self.plan_v2_stmt(stmt.into())?.unwrap();
        Ok(var)
    }
}
