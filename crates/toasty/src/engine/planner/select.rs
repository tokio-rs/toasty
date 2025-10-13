use super::{plan, Planner, Result};
use toasty_core::stmt;

impl Planner<'_> {
    pub(super) fn plan_stmt_select(&mut self, stmt: stmt::Query) -> Result<plan::VarId> {
        // New planner
        let mut stmt = stmt::Statement::Query(stmt);

        // Lower the statement
        self.lower_stmt(&mut stmt);

        return self.plan_v2_stmt(stmt);
    }
}
