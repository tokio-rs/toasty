use super::*;

impl Planner<'_> {
    /// Walk the statement, find subqueries, and plan them independently.
    ///
    /// At this point, the expression should have been simplified to the point
    /// that subqueries are actually required to be executed separately.
    pub(super) fn plan_subqueries<T: stmt::Node>(
        &mut self,
        stmt: &mut T,
    ) -> Result<Vec<plan::InputSource>> {
        let mut sources = vec![];
        let mut err = None;

        stmt::visit_mut::for_each_expr_mut(stmt, |expr| {
            if expr.is_in_subquery() {
                let stmt::Expr::InSubquery(expr_in_subquery) = expr.take() else {
                    panic!()
                };

                let base = *expr_in_subquery.expr;
                let query = *expr_in_subquery.query;

                // Replace the InSubquery with an InList expression
                let arg = stmt::Expr::arg(sources.len());
                *expr = stmt::Expr::in_list(base, arg);

                match self.plan_stmt_select(&Context::default(), query) {
                    Ok(output) => {
                        sources.push(plan::InputSource::Value(output));
                    }
                    Err(e) => err = Some(e),
                }
            }
        });

        if let Some(err) = err {
            Err(err)
        } else {
            Ok(sources)
        }
    }
}
