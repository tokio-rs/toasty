use super::*;

use stmt::Expr;

impl VisitMut for SimplifyExpr<'_> {
    fn visit_stmt_mut(&mut self, _i: &mut stmt::Statement) {
        panic!("should not be reached");
    }

    fn visit_expr_project_mut(&mut self, i: &mut stmt::ExprProject) {
        assert!(i.projection.len() <= 1);
    }

    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        // First, simplify the expression.
        stmt::visit_mut::visit_expr_mut(self, i);

        // If an in-subquery expression, then try lifting it.
        let maybe_expr = match i {
            Expr::BinaryOp(expr_binary_op) => self.simplify_expr_binary_op(
                expr_binary_op.op,
                &mut *expr_binary_op.lhs,
                &mut *expr_binary_op.rhs,
            ),
            Expr::Cast(expr) => self.simplify_expr_cast(expr),
            Expr::InList(expr) => self.simplify_expr_in_list(expr),
            Expr::InSubquery(expr_in_subquery) => {
                self.lift_in_subquery(&expr_in_subquery.expr, &expr_in_subquery.query)
            }
            Expr::Record(expr) => self.simplify_expr_record(expr),
            Expr::IsNull(_) => todo!(),
            _ => None,
        };

        if let Some(expr) = maybe_expr {
            *i = expr;
        }
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt) {
        SimplifyStmt {
            schema: self.schema,
        }
        .visit_stmt_mut(&mut *i.stmt);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut stmt::Query) {
        SimplifyStmt {
            schema: self.schema,
        }
        .visit_stmt_query_mut(i);
    }
}
