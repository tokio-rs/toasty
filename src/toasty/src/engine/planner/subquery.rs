use super::*;

struct PlanSubqueries<'a, 'b, 'stmt> {
    planner: &'b mut Planner<'a, 'stmt>,
}

impl<'a, 'stmt> Planner<'a, 'stmt> {
    /// Walk the statement, find subqueries, and plan them independently.
    ///
    /// At this point, the expression should have been simplified to the point
    /// that subqueries are actually required to be executed separately.
    pub(super) fn plan_subqueries<T: stmt::Node<'stmt>>(&mut self, stmt: &T) {
        PlanSubqueries { planner: self }.visit(stmt);
    }

    pub(super) fn subquery_var(&self, subquery: &stmt::Query<'stmt>) -> plan::VarId {
        self.subqueries
            .get(&(subquery as *const _ as usize))
            .copied()
            .unwrap()
    }
}

impl<'stmt> stmt::Visit<'stmt> for PlanSubqueries<'_, '_, 'stmt> {
    fn visit_expr_in_subquery(&mut self, i: &stmt::ExprInSubquery<'stmt>) {
        stmt::visit::visit_expr_in_subquery(self, i);

        // The subquery has already been simplified
        // TODO: don't clone
        let output = self
            .planner
            .plan_simplified_select(&select::Context::default(), (*i.query).clone());

        // Track the output of the subquery
        self.planner
            .subqueries
            .insert(&*i.query as *const _ as usize, output);
    }
}
