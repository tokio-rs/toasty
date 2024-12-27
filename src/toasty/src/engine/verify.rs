use toasty_core::{
    schema::ModelId,
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
        VerifyExpr {
            schema: self.schema,
            model: i.from.as_model_id(),
        }
        .verify_filter(&i.filter);
    }

    fn visit_stmt_select(&mut self, i: &stmt::Select) {
        VerifyExpr {
            schema: self.schema,
            model: i.source.as_model_id(),
        }
        .verify_filter(&i.filter);
    }

    fn visit_stmt_update(&mut self, i: &stmt::Update) {
        // Is not an empty update
        assert!(!i.assignments.is_empty(), "stmt = {i:#?}");

        let mut verify_expr = VerifyExpr {
            schema: self.schema,
            model: i.target.as_model_id(),
        };

        verify_expr.visit_stmt_update(i);
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
        let _ = i.resolve_field(self.schema, self.schema.model(self.model));
    }

    fn visit_expr_binary_op(&mut self, i: &stmt::ExprBinaryOp) {
        stmt::visit::visit_expr_binary_op(self, i);

        /*
        if i.op.is_in_set() {
            assert!(!matches!(&*i.rhs, stmt::Expr::Record(_)));
        } else {
            // TODO: Update this verification
            /*
            assert!(
                lhs_ty.casts_to(&rhs_ty),
                "lhs_ty={:#?}; rhs_ty={:#?}",
                lhs_ty,
                rhs_ty
            );
            assert!(lhs_ty.applies_binary_op(i.op));
            */
        }
        */
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

        // TODO: update this verification
        /*
        // The subquery should be a list of the same type as expression
        let expr_ty = self.ty(&i.expr);
        // let subquery_ty = i.subquery.ty(self.schema);
        let subquery_ty = todo!();

        assert_eq!(
            expr_ty,
            match subquery_ty {
                stmt::Type::List(item) => *item,
                _ => todo!(),
            }
        );
        */
    }
}
