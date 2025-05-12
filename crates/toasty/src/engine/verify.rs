use toasty_core::{
    schema::app::ModelId,
    stmt::{self, Visit},
};

use super::*;

struct Verify<'a> {
    schema: &'a Schema,
}

struct VerifyExpr<'a> {
    schema: &'a Schema,
    model: ModelId,
}

pub(crate) fn apply(schema: &Schema, stmt: &Statement) {
    Verify { schema }.visit(stmt);
}

impl stmt::Visit for Verify<'_> {
    fn visit_stmt_delete(&mut self, i: &stmt::Delete) {
        stmt::visit::visit_stmt_delete(self, i);

        VerifyExpr {
            schema: self.schema,
            model: i.from.as_model_id(),
        }
        .verify_filter(&i.filter);
    }

    fn visit_stmt_query(&mut self, i: &stmt::Query) {
        stmt::visit::visit_stmt_query(self, i);

        self.verify_offset_key_matches_order_by(i);
    }

    fn visit_stmt_select(&mut self, i: &stmt::Select) {
        stmt::visit::visit_stmt_select(self, i);

        VerifyExpr {
            schema: self.schema,
            model: i.source.as_model_id(),
        }
        .verify_filter(&i.filter);
    }

    fn visit_stmt_update(&mut self, i: &stmt::Update) {
        stmt::visit::visit_stmt_update(self, i);

        // Is not an empty update
        assert!(!i.assignments.is_empty(), "stmt = {i:#?}");

        let mut verify_expr = VerifyExpr {
            schema: self.schema,
            model: i.target.as_model_id(),
        };

        verify_expr.visit_stmt_update(i);
    }
}

impl Verify<'_> {
    fn verify_offset_key_matches_order_by(&self, i: &stmt::Query) {
        let Some(limit) = i.limit.as_ref() else {
            return;
        };

        let Some(stmt::Offset::After(offset)) = limit.offset.as_ref() else {
            return;
        };

        let Some(order_by) = i.order_by.as_ref() else {
            todo!("specified offset but no order; stmt={i:#?}");
        };

        match offset {
            stmt::Expr::Value(stmt::Value::Record(record)) => {
                assert!(
                    order_by.exprs.len() == record.fields.len(),
                    "order_by = {order_by:#?}"
                );
            }
            stmt::Expr::Value(_) => {
                assert!(order_by.exprs.len() == 1, "order_by = {order_by:#?}");
            }
            _ => todo!("unsupported offset expression; stmt={i:#?}"),
        }
    }
}

impl VerifyExpr<'_> {
    fn verify_filter(&mut self, expr: &stmt::Expr) {
        self.assert_bool_expr(expr);
        self.visit(expr);
    }

    fn assert_bool_expr(&self, expr: &stmt::Expr) {
        use stmt::Expr::*;

        match expr {
            And(_)
            | BinaryOp(_)
            | InList(_)
            | InSubquery(_)
            | Or(_)
            | Value(stmt::Value::Bool(_)) => {}
            expr => panic!("Not a bool? {expr:#?}"),
        }
    }
}

impl stmt::Visit for VerifyExpr<'_> {
    fn visit_expr_and(&mut self, i: &stmt::ExprAnd) {
        stmt::visit::visit_expr_and(self, i);

        for expr in &i.operands {
            self.assert_bool_expr(expr);
        }
    }

    fn visit_expr_or(&mut self, i: &stmt::ExprOr) {
        stmt::visit::visit_expr_or(self, i);

        for expr in &i.operands {
            self.assert_bool_expr(expr);
        }
    }

    fn visit_projection(&mut self, i: &stmt::Projection) {
        // The path should resolve. Verifying type is done at a higher level
        let _ = i.resolve_field(&self.schema.app, self.schema.app.model(self.model));
    }

    fn visit_expr_binary_op(&mut self, i: &stmt::ExprBinaryOp) {
        stmt::visit::visit_expr_binary_op(self, i);
    }

    fn visit_expr_in_subquery(&mut self, i: &stmt::ExprInSubquery) {
        // stmt::visit::visit_expr_in_subquery(self, i);

        // Visit **only** the subquery expression
        self.visit(&*i.expr);

        // The subquery is verified independently
        Verify {
            schema: self.schema,
        }
        .visit(&*i.query);
    }
}
