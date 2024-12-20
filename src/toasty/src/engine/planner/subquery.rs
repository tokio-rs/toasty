use super::*;

impl Planner<'_> {
    /// Walk the statement, find subqueries, and plan them independently.
    ///
    /// At this point, the expression should have been simplified to the point
    /// that subqueries are actually required to be executed separately.
    pub(super) fn plan_subqueries<T: stmt::Node>(&mut self, stmt: &mut T) {
        stmt::visit_mut::for_each_expr_mut(stmt, |expr| {
            if let stmt::Expr::InSubquery(expr) = expr {
                // The subquery has already been simplified
                // TODO: don't clone
                let output =
                    self.plan_simplified_select(&select::Context::default(), (*expr.query).clone());

                // Track the output of the subquery
                self.subqueries
                    .insert(&*expr.query as *const _ as usize, output);
            }
        });
    }

    pub(super) fn subquery_var(&self, subquery: &stmt::Query) -> plan::VarId {
        self.subqueries
            .get(&(subquery as *const _ as usize))
            .copied()
            .unwrap()
    }
}
