use super::{plan, Planner, Result};
use toasty_core::stmt;

impl Planner<'_> {
    pub(super) fn plan_stmt_select(&mut self, stmt: stmt::Query) -> Result<plan::VarId> {
        // New planner
        let mut stmt = stmt::Statement::Query(stmt);

        // TODO: don't unwrap once vars can store more than just ValueStream
        let var = self.plan_v2_stmt(stmt)?.unwrap();
        Ok(var)
    }
}
