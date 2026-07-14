use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    /// Heavyweight `List` rewrites. Cheap canonicalization (collapsing a
    /// list of literal values into `Value::List`) runs in
    /// `fold::expr_list` before this is reached.
    pub(super) fn simplify_expr_list(&mut self, expr: &mut stmt::ExprList) -> Option<stmt::Expr> {
        self.simplify_expr_list_insert_stmt(expr)
    }

    // TODO: rewrite this
    fn simplify_expr_list_insert_stmt(&mut self, expr: &mut stmt::ExprList) -> Option<stmt::Expr> {
        // Check if all items are Expr::Stmt with single-row Insert statements
        for item in &expr.items {
            let stmt::Expr::Stmt(expr_stmt) = item else {
                return None;
            };

            let insert = expr_stmt.stmt.as_insert()?;

            // Must be single-row insert
            if !insert.source.single {
                return None;
            }

            // Must have Returning::Model
            match &insert.returning {
                Some(stmt::Returning::Model { .. }) => {}
                _ => return None,
            }

            // Must target a Model (not Table or Scope)
            if !insert.target.is_model() {
                return None;
            }
        }

        // Extract the first insert to get the target model and returning clause
        let first_insert = match &expr.items[0] {
            stmt::Expr::Stmt(s) => s.stmt.as_insert().unwrap(),
            _ => unreachable!(),
        };

        let first_target_model = match &first_insert.target {
            stmt::InsertTarget::Model(model_id) => model_id,
            _ => unreachable!(),
        };

        let first_returning = first_insert.returning.as_ref().unwrap();

        // Check all inserts target the same model and have the same returning clause
        for item in &expr.items[1..] {
            let insert = match item {
                stmt::Expr::Stmt(s) => s.stmt.as_insert().unwrap(),
                _ => unreachable!(),
            };

            let target_model = match &insert.target {
                stmt::InsertTarget::Model(model_id) => model_id,
                _ => unreachable!(),
            };

            if target_model != first_target_model {
                return None;
            }

            if insert.returning.as_ref().unwrap() != first_returning {
                return None;
            }
        }

        // All inserts are compatible, merge them into a single batch insert
        let mut items = expr.items.drain(..).collect::<Vec<_>>();
        let mut merged_insert = match items.remove(0) {
            stmt::Expr::Stmt(s) => s.stmt.into_insert_unwrap(),
            _ => unreachable!(),
        };

        for item in items {
            let insert = match item {
                stmt::Expr::Stmt(s) => s.stmt.into_insert_unwrap(),
                _ => unreachable!(),
            };
            merged_insert.merge(insert);
        }

        // Set single = false since we're now returning a list of records
        merged_insert.source.single = false;

        Some(stmt::Expr::stmt(stmt::Statement::Insert(merged_insert)))
    }
}
